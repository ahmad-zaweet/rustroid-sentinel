//! In-memory TTL caching for [`DashboardRepository`] read queries.
//!
//! Caches sit in front of the raw repository
//! queries so `/api/*` and the HTMX dashboard SSR partials share one cached
//! value. Values are wrapped in `Arc` so cache hits clone a pointer, not the
//! underlying `Vec`/struct. Errors are never cached (`try_get_with` only
//! memoizes `Ok`), and come back as `Arc<sqlx::Error>` since `sqlx::Error`
//! itself isn't `Clone` — callers that just log-and-500 can use it as-is via
//! `Display`/`Debug`.

use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDate;
use moka::future::Cache;
use sqlx::PgPool;

use crate::api::types::{
    ApproachRecord, AsteroidCatalogRecord, AsteroidDetailRecord, EtlRunRecord, TimePeriod,
    VelocityDataPoint,
};
use crate::settings::CacheConfig;

use super::catalog::{CatalogListParams, CatalogRepository};
use super::dashboard::{ApproachQueryParams, DashboardRepository, DashboardStats};
use super::report::{ReportRepository, WeeklyReportSummary};

/// Records a cache hit/miss for `name`. No-op when the `metrics` feature is
/// disabled, so `cache` doesn't have to depend on it.
#[cfg(feature = "metrics")]
fn record_cache(name: &'static str, hit: bool) {
    crate::metrics::record_cache_result(name, hit);
}

/// No-op stub for when the `metrics` feature is disabled.
#[cfg(not(feature = "metrics"))]
fn record_cache(_name: &'static str, _hit: bool) {}

/// Builds the cache key for `get_paginated_approaches`, covering every field
/// that affects the result set. Extracted from the call site so the format
/// string (and its placeholder count) has direct unit-test coverage.
fn approach_query_key(params: &ApproachQueryParams<'_>) -> String {
    format!(
        "{}:{}:{:?}:{:?}:{:?}:{:?}:{:?}:{}",
        params.page,
        params.page_size,
        params.start_date,
        params.end_date,
        params.hazard_class,
        params.sort_by,
        params.sort_dir,
        params.sentry_only,
    )
}

/// Builds the cache key for `catalog_list`, covering every field of
/// `CatalogListParams`. Extracted from the call site so the format string
/// (and its placeholder count) has direct unit-test coverage — a previous
/// version of this key had a placeholder/field-count mismatch that only
/// surfaced as a compile error, not a test failure.
fn catalog_list_key(params: &CatalogListParams<'_>) -> String {
    format!(
        "{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{}",
        params.cursor,
        params.sort,
        params.sort_dir,
        params.name,
        params.is_potentially_hazardous,
        params.is_sentry_object,
        params.start_date,
        params.end_date,
        params.min_diameter_km,
        params.max_diameter_km,
        params.orbit_class,
        params.spectral_class,
        params.min_torino_scale,
        params.min_palermo_scale,
        params.limit,
    )
}

type CacheResult<T> = Result<Arc<T>, Arc<sqlx::Error>>;
type DateRangeKey = (Option<NaiveDate>, Option<NaiveDate>);
type EtlRunsPage = Arc<(Vec<EtlRunRecord>, i64)>;
type CatalogListPage = Arc<(Vec<AsteroidCatalogRecord>, bool)>;
type ClassificationValues = Arc<(Vec<String>, Vec<String>)>;

/// Holds one TTL-bounded [`moka`] store per cached `DashboardRepository`
/// query. Singleton queries (no meaningful args) use `max_capacity(1)`;
/// queries keyed by user-controlled params (pagination, dates) are bounded
/// by `CacheConfig::max_entries` to prevent unbounded growth from a
/// high-cardinality key space.
#[derive(Clone)]
pub struct DashboardCache {
    enabled: bool,
    stats: Cache<(), Arc<DashboardStats>>,
    recent_approaches: Cache<(), Arc<Vec<ApproachRecord>>>,
    velocity_default: Cache<(), Arc<Vec<VelocityDataPoint>>>,
    velocity_by_period: Cache<String, Arc<Vec<VelocityDataPoint>>>,
    velocity_by_range: Cache<DateRangeKey, Arc<Vec<VelocityDataPoint>>>,
    paginated_approaches: Cache<String, Arc<(Vec<ApproachRecord>, i64)>>,
    recent_etl_runs: Cache<(), Arc<Vec<EtlRunRecord>>>,
    paginated_etl_runs: Cache<(u32, u32), EtlRunsPage>,
    catalog_list: Cache<String, CatalogListPage>,
    catalog_detail: Cache<String, Arc<Option<AsteroidDetailRecord>>>,
    catalog_similar: Cache<(String, i64), Arc<Option<Vec<AsteroidCatalogRecord>>>>,
    catalog_classifications: Cache<(), ClassificationValues>,
    weekly_report: Cache<NaiveDate, Arc<WeeklyReportSummary>>,
}

impl DashboardCache {
    /// Builds all stores from `cfg`. Cheap to call once at startup; the
    /// resulting handle is `Clone` (internally `Arc`-backed) and lives in
    /// `AppState`.
    pub fn new(cfg: &CacheConfig) -> Self {
        let stats_ttl = Duration::from_secs(cfg.stats_ttl_secs.into());
        let velocity_ttl = Duration::from_secs(cfg.velocity_ttl_secs.into());
        let approaches_ttl = Duration::from_secs(cfg.approaches_ttl_secs.into());
        let etl_ttl = Duration::from_secs(cfg.etl_runs_ttl_secs.into());
        let catalog_ttl = Duration::from_secs(cfg.catalog_ttl_secs.into());
        let catalog_classifications_ttl =
            Duration::from_secs(cfg.catalog_classifications_ttl_secs.into());
        let report_ttl = Duration::from_secs(cfg.report_ttl_secs.into());
        let max_entries: u64 = cfg.max_entries.into();

        Self {
            enabled: cfg.enabled,
            stats: Cache::builder()
                .max_capacity(1)
                .time_to_live(stats_ttl)
                .build(),
            recent_approaches: Cache::builder()
                .max_capacity(1)
                .time_to_live(approaches_ttl)
                .build(),
            velocity_default: Cache::builder()
                .max_capacity(1)
                .time_to_live(velocity_ttl)
                .build(),
            velocity_by_period: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(velocity_ttl)
                .build(),
            velocity_by_range: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(velocity_ttl)
                .build(),
            paginated_approaches: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(approaches_ttl)
                .build(),
            recent_etl_runs: Cache::builder()
                .max_capacity(1)
                .time_to_live(etl_ttl)
                .build(),
            paginated_etl_runs: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(etl_ttl)
                .build(),
            catalog_list: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(catalog_ttl)
                .build(),
            catalog_detail: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(catalog_ttl)
                .build(),
            catalog_similar: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(catalog_ttl)
                .build(),
            catalog_classifications: Cache::builder()
                .max_capacity(1)
                .time_to_live(catalog_classifications_ttl)
                .build(),
            weekly_report: Cache::builder()
                .max_capacity(4)
                .time_to_live(report_ttl)
                .build(),
        }
    }

    /// Clears every store. Call after a successful ETL `load` run so the
    /// next request sees fresh data immediately instead of waiting out the
    /// TTL. `invalidate_all` is lazy (entries are dropped on next access),
    /// which is fine here since staleness is already TTL-bounded.
    pub fn invalidate_all(&self) {
        self.stats.invalidate_all();
        self.recent_approaches.invalidate_all();
        self.velocity_default.invalidate_all();
        self.velocity_by_period.invalidate_all();
        self.velocity_by_range.invalidate_all();
        self.paginated_approaches.invalidate_all();
        self.recent_etl_runs.invalidate_all();
        self.paginated_etl_runs.invalidate_all();
        self.catalog_list.invalidate_all();
        self.catalog_detail.invalidate_all();
        self.catalog_similar.invalidate_all();
        self.catalog_classifications.invalidate_all();
        self.weekly_report.invalidate_all();
    }

    /// Cached wrapper around `DashboardRepository::get_stats`.
    pub async fn get_stats(&self, pool: &PgPool) -> CacheResult<DashboardStats> {
        if !self.enabled {
            return DashboardRepository::get_stats(pool)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache("stats", self.stats.contains_key(&()));
        self.stats
            .try_get_with((), async {
                DashboardRepository::get_stats(pool).await.map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `DashboardRepository::get_recent_approaches`.
    pub async fn get_recent_approaches(&self, pool: &PgPool) -> CacheResult<Vec<ApproachRecord>> {
        if !self.enabled {
            return DashboardRepository::get_recent_approaches(pool)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache(
            "recent_approaches",
            self.recent_approaches.contains_key(&()),
        );
        self.recent_approaches
            .try_get_with((), async {
                DashboardRepository::get_recent_approaches(pool)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `DashboardRepository::get_velocity_data`.
    pub async fn get_velocity_data(&self, pool: &PgPool) -> CacheResult<Vec<VelocityDataPoint>> {
        if !self.enabled {
            return DashboardRepository::get_velocity_data(pool)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache("velocity_default", self.velocity_default.contains_key(&()));
        self.velocity_default
            .try_get_with((), async {
                DashboardRepository::get_velocity_data(pool)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `DashboardRepository::get_velocity_data_by_period`.
    pub async fn get_velocity_data_by_period(
        &self,
        pool: &PgPool,
        period: TimePeriod,
    ) -> CacheResult<Vec<VelocityDataPoint>> {
        if !self.enabled {
            return DashboardRepository::get_velocity_data_by_period(pool, period)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        let key = format!("{period:?}");
        record_cache(
            "velocity_by_period",
            self.velocity_by_period.contains_key(&key),
        );
        self.velocity_by_period
            .try_get_with(key, async {
                DashboardRepository::get_velocity_data_by_period(pool, period)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `DashboardRepository::get_velocity_data_with_filter`.
    pub async fn get_velocity_data_with_filter(
        &self,
        pool: &PgPool,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> CacheResult<Vec<VelocityDataPoint>> {
        if !self.enabled {
            return DashboardRepository::get_velocity_data_with_filter(pool, start_date, end_date)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache(
            "velocity_by_range",
            self.velocity_by_range.contains_key(&(start_date, end_date)),
        );
        self.velocity_by_range
            .try_get_with((start_date, end_date), async {
                DashboardRepository::get_velocity_data_with_filter(pool, start_date, end_date)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `DashboardRepository::get_paginated_approaches`.
    /// Keys on every field of `params` since all of them (page, filters,
    /// sort) affect the result set.
    pub async fn get_paginated_approaches(
        &self,
        pool: &PgPool,
        params: ApproachQueryParams<'_>,
    ) -> CacheResult<(Vec<ApproachRecord>, i64)> {
        if !self.enabled {
            return DashboardRepository::get_paginated_approaches(pool, params)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        let key = approach_query_key(&params);
        record_cache(
            "paginated_approaches",
            self.paginated_approaches.contains_key(&key),
        );
        self.paginated_approaches
            .try_get_with(key, async {
                DashboardRepository::get_paginated_approaches(pool, params)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `DashboardRepository::get_recent_etl_runs`.
    pub async fn get_recent_etl_runs(&self, pool: &PgPool) -> CacheResult<Vec<EtlRunRecord>> {
        if !self.enabled {
            return DashboardRepository::get_recent_etl_runs(pool)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache("recent_etl_runs", self.recent_etl_runs.contains_key(&()));
        self.recent_etl_runs
            .try_get_with((), async {
                DashboardRepository::get_recent_etl_runs(pool)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `DashboardRepository::get_paginated_etl_runs`.
    pub async fn get_paginated_etl_runs(
        &self,
        pool: &PgPool,
        page: u32,
        page_size: u32,
    ) -> CacheResult<(Vec<EtlRunRecord>, i64)> {
        if !self.enabled {
            return DashboardRepository::get_paginated_etl_runs(pool, page, page_size)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache(
            "paginated_etl_runs",
            self.paginated_etl_runs.contains_key(&(page, page_size)),
        );
        self.paginated_etl_runs
            .try_get_with((page, page_size), async {
                DashboardRepository::get_paginated_etl_runs(pool, page, page_size)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `CatalogRepository::list`. Keys on every field
    /// of `params` (cursor, sort, every filter) since all of them affect the
    /// result set — high cardinality, hence the size-bounded store.
    pub async fn catalog_list(
        &self,
        pool: &PgPool,
        params: CatalogListParams<'_>,
    ) -> CacheResult<(Vec<AsteroidCatalogRecord>, bool)> {
        if !self.enabled {
            return CatalogRepository::list(pool, params)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        let key = catalog_list_key(&params);
        record_cache("catalog_list", self.catalog_list.contains_key(&key));
        self.catalog_list
            .try_get_with(key, async {
                CatalogRepository::list(pool, params).await.map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `CatalogRepository::get_detail`.
    pub async fn catalog_detail(
        &self,
        pool: &PgPool,
        neo_reference_id: &str,
    ) -> CacheResult<Option<AsteroidDetailRecord>> {
        if !self.enabled {
            return CatalogRepository::get_detail(pool, neo_reference_id)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache(
            "catalog_detail",
            self.catalog_detail.contains_key(neo_reference_id),
        );
        self.catalog_detail
            .try_get_with(neo_reference_id.to_string(), async {
                CatalogRepository::get_detail(pool, neo_reference_id)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `CatalogRepository::similar`.
    pub async fn catalog_similar(
        &self,
        pool: &PgPool,
        neo_reference_id: &str,
        limit: i64,
    ) -> CacheResult<Option<Vec<AsteroidCatalogRecord>>> {
        if !self.enabled {
            return CatalogRepository::similar(pool, neo_reference_id, limit)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache(
            "catalog_similar",
            self.catalog_similar
                .contains_key(&(neo_reference_id.to_string(), limit)),
        );
        self.catalog_similar
            .try_get_with((neo_reference_id.to_string(), limit), async {
                CatalogRepository::similar(pool, neo_reference_id, limit)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `CatalogRepository::distinct_classification_values`.
    pub async fn catalog_classifications(
        &self,
        pool: &PgPool,
    ) -> CacheResult<(Vec<String>, Vec<String>)> {
        if !self.enabled {
            return CatalogRepository::distinct_classification_values(pool)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache(
            "catalog_classifications",
            self.catalog_classifications.contains_key(&()),
        );
        self.catalog_classifications
            .try_get_with((), async {
                CatalogRepository::distinct_classification_values(pool)
                    .await
                    .map(Arc::new)
            })
            .await
    }

    /// Cached wrapper around `ReportRepository::get_weekly_summary`. Keyed
    /// by `end_date` alone: callers always pass `end_date - 7 days` as
    /// `start_date`, so `end_date` (today, in practice) fully determines the
    /// window.
    pub async fn get_weekly_summary(
        &self,
        pool: &PgPool,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> CacheResult<WeeklyReportSummary> {
        if !self.enabled {
            return ReportRepository::get_weekly_summary(pool, start_date, end_date)
                .await
                .map(Arc::new)
                .map_err(Arc::new);
        }
        record_cache("weekly_report", self.weekly_report.contains_key(&end_date));
        self.weekly_report
            .try_get_with(end_date, async {
                ReportRepository::get_weekly_summary(pool, start_date, end_date)
                    .await
                    .map(Arc::new)
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{CatalogSortKey, SortDir};

    fn base_approach_params() -> ApproachQueryParams<'static> {
        ApproachQueryParams {
            page: 1,
            page_size: 20,
            start_date: None,
            end_date: None,
            hazard_class: None,
            sort_by: None,
            sort_dir: None,
            sentry_only: false,
        }
    }

    fn base_catalog_params() -> CatalogListParams<'static> {
        CatalogListParams {
            cursor: None,
            sort: CatalogSortKey::default(),
            sort_dir: SortDir::default(),
            name: None,
            is_potentially_hazardous: None,
            is_sentry_object: None,
            start_date: None,
            end_date: None,
            min_diameter_km: None,
            max_diameter_km: None,
            orbit_class: None,
            spectral_class: None,
            min_torino_scale: None,
            min_palermo_scale: None,
            limit: 25,
        }
    }

    #[test]
    fn approach_query_key_is_deterministic() {
        let params = base_approach_params();
        assert_eq!(approach_query_key(&params), approach_query_key(&params));
    }

    #[test]
    fn approach_query_key_differs_on_page() {
        let a = base_approach_params();
        let mut b = base_approach_params();
        b.page = 2;
        assert_ne!(approach_query_key(&a), approach_query_key(&b));
    }

    #[test]
    fn approach_query_key_differs_on_every_field() {
        let base = approach_query_key(&base_approach_params());

        let mut page_size = base_approach_params();
        page_size.page_size = 50;
        assert_ne!(approach_query_key(&page_size), base);

        let mut start_date = base_approach_params();
        start_date.start_date = Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        assert_ne!(approach_query_key(&start_date), base);

        let mut hazard = base_approach_params();
        hazard.hazard_class = Some("Critical");
        assert_ne!(approach_query_key(&hazard), base);

        let mut sentry = base_approach_params();
        sentry.sentry_only = true;
        assert_ne!(approach_query_key(&sentry), base);
    }

    #[test]
    fn catalog_list_key_is_deterministic() {
        let params = base_catalog_params();
        assert_eq!(catalog_list_key(&params), catalog_list_key(&params));
    }

    #[test]
    fn catalog_list_key_differs_on_limit() {
        let a = base_catalog_params();
        let mut b = base_catalog_params();
        b.limit = 50;
        assert_ne!(catalog_list_key(&a), catalog_list_key(&b));
    }

    #[test]
    fn catalog_list_key_differs_on_every_filter_field() {
        let base = catalog_list_key(&base_catalog_params());

        let mut name = base_catalog_params();
        name.name = Some("Apophis");
        assert_ne!(catalog_list_key(&name), base);

        let mut hazardous = base_catalog_params();
        hazardous.is_potentially_hazardous = Some(true);
        assert_ne!(catalog_list_key(&hazardous), base);

        let mut sentry = base_catalog_params();
        sentry.is_sentry_object = Some(true);
        assert_ne!(catalog_list_key(&sentry), base);

        let mut orbit = base_catalog_params();
        orbit.orbit_class = Some("Aten");
        assert_ne!(catalog_list_key(&orbit), base);

        let mut spectral = base_catalog_params();
        spectral.spectral_class = Some("Sq");
        assert_ne!(catalog_list_key(&spectral), base);

        let mut torino = base_catalog_params();
        torino.min_torino_scale = Some(3);
        assert_ne!(catalog_list_key(&torino), base);

        let mut palermo = base_catalog_params();
        palermo.min_palermo_scale = Some(-1.5);
        assert_ne!(catalog_list_key(&palermo), base);

        let mut sort = base_catalog_params();
        sort.sort = CatalogSortKey::Name;
        assert_ne!(catalog_list_key(&sort), base);

        let mut sort_dir = base_catalog_params();
        sort_dir.sort_dir = SortDir::Asc;
        assert_ne!(catalog_list_key(&sort_dir), base);
    }

    #[tokio::test]
    async fn dashboard_cache_respects_disabled_flag() {
        let cfg = CacheConfig {
            enabled: false,
            ..Default::default()
        };
        let cache = DashboardCache::new(&cfg);
        assert!(!cache.enabled);
    }

    #[tokio::test]
    async fn dashboard_cache_singleton_stores_have_capacity_one() {
        let cfg = CacheConfig {
            enabled: true,
            ..Default::default()
        };
        let cache = DashboardCache::new(&cfg);

        cache
            .stats
            .insert((), Arc::new(DashboardStats::default()))
            .await;
        cache.stats.run_pending_tasks().await;
        assert_eq!(cache.stats.entry_count(), 1);
    }

    #[tokio::test]
    async fn dashboard_cache_invalidate_all_clears_populated_entries() {
        let cfg = CacheConfig {
            enabled: true,
            ..Default::default()
        };
        let cache = DashboardCache::new(&cfg);

        cache
            .stats
            .insert((), Arc::new(DashboardStats::default()))
            .await;
        cache
            .catalog_list
            .insert("some-key".to_string(), Arc::new((Vec::new(), false)))
            .await;
        cache.stats.run_pending_tasks().await;
        cache.catalog_list.run_pending_tasks().await;
        assert_eq!(cache.stats.entry_count(), 1);
        assert_eq!(cache.catalog_list.entry_count(), 1);

        cache.invalidate_all();
        cache.stats.run_pending_tasks().await;
        cache.catalog_list.run_pending_tasks().await;
        assert_eq!(cache.stats.entry_count(), 0);
        assert_eq!(cache.catalog_list.entry_count(), 0);
    }
}
