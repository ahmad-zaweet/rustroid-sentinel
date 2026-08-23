//! Asteroid catalog handlers: keyset-paginated JSON listing/detail
//! (`/api/asteroids`), and the matching HTMX SSR page/partial
//! (`/dashboard/catalog`).

use askama::Template;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use tracing::error;

use crate::api::cursor::{CatalogCursor, CursorValue};
use crate::api::templates::{CatalogDetailTemplate, CatalogRowsTemplate, CatalogTemplate};
use crate::api::types::{
    ApiResponse, AsteroidCatalogRecord, AsteroidDetailRecord, CatalogQuery, CatalogSortKey,
    CursorPage, SortDir,
};
use crate::database::catalog::{CatalogListParams, CatalogRepository};
use crate::server::AppState;

/// Rows per catalog page, both for the JSON API and the HTMX partial.
const CATALOG_PAGE_SIZE: u32 = 50;

/// Number of nearest neighbors returned by the similar-asteroids lookup.
const SIMILAR_LIMIT: i64 = 5;

/// Decodes `query.cursor`, if present.
///
/// # Errors
///
/// Returns `StatusCode::BAD_REQUEST` if `cursor` is set but doesn't decode,
/// or decodes to a different sort key/direction than `query` requests — a
/// client-supplied cursor should never be silently ignored or reinterpreted
/// against the wrong ordering, since either would quietly corrupt or restart
/// pagination.
fn decode_cursor(query: &CatalogQuery) -> Result<Option<CatalogCursor>, StatusCode> {
    let cursor = query
        .cursor
        .as_deref()
        .map(CatalogCursor::decode)
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(cursor) = &cursor
        && (cursor.sort != query.sort || cursor.sort_dir != query.sort_dir)
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(cursor)
}

fn list_params(query: &CatalogQuery, cursor: Option<CatalogCursor>) -> CatalogListParams<'_> {
    CatalogListParams {
        cursor,
        sort: query.sort,
        sort_dir: query.sort_dir,
        name: query.name.as_deref().filter(|s| !s.is_empty()),
        is_potentially_hazardous: query.is_potentially_hazardous,
        is_sentry_object: query.is_sentry_object,
        start_date: query.start_date,
        end_date: query.end_date,
        min_diameter_km: query.min_diameter_km,
        max_diameter_km: query.max_diameter_km,
        orbit_class: query.orbit_class.as_deref().filter(|s| !s.is_empty()),
        spectral_class: query.spectral_class.as_deref().filter(|s| !s.is_empty()),
        min_torino_scale: query.min_torino_scale,
        min_palermo_scale: query.min_palermo_scale,
        limit: CATALOG_PAGE_SIZE,
    }
}

/// Serialized form of a [`CatalogSortKey`], for template rendering.
fn sort_key_str(sort: CatalogSortKey) -> &'static str {
    match sort {
        CatalogSortKey::ApproachActivity => "approach_activity",
        CatalogSortKey::Name => "name",
        CatalogSortKey::Diameter => "diameter",
        CatalogSortKey::Torino => "torino",
        CatalogSortKey::Palermo => "palermo",
    }
}

/// Serialized form of a [`SortDir`], for template rendering.
fn sort_dir_str(dir: SortDir) -> &'static str {
    match dir {
        SortDir::Asc => "asc",
        SortDir::Desc => "desc",
    }
}

/// Builds a query string from `query`'s non-empty filter values (excludes
/// `cursor`/`sort`/`sort_dir`, which sort-header and pagination links append
/// separately), so sort-header and Prev/Next links can preserve the active
/// filters. Mirrors `build_filter_query_string` in
/// `src/api/handlers/dashboard.rs`.
fn build_catalog_query_string(query: &CatalogQuery) -> String {
    let mut parts = Vec::new();

    if let Some(name) = query.name.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("name={}", urlencoding::encode(name)));
    }
    if let Some(orbit_class) = query.orbit_class.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("orbit_class={}", urlencoding::encode(orbit_class)));
    }
    if let Some(spectral_class) = query.spectral_class.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!(
            "spectral_class={}",
            urlencoding::encode(spectral_class)
        ));
    }
    if query.is_potentially_hazardous.unwrap_or(false) {
        parts.push("is_potentially_hazardous=true".to_string());
    }
    if query.is_sentry_object.unwrap_or(false) {
        parts.push("is_sentry_object=true".to_string());
    }
    if let Some(start) = query.start_date {
        parts.push(format!("start_date={start}"));
    }
    if let Some(end) = query.end_date {
        parts.push(format!("end_date={end}"));
    }
    if let Some(min_d) = query.min_diameter_km {
        parts.push(format!("min_diameter_km={min_d}"));
    }
    if let Some(max_d) = query.max_diameter_km {
        parts.push(format!("max_diameter_km={max_d}"));
    }
    if let Some(min_torino) = query.min_torino_scale {
        parts.push(format!("min_torino_scale={min_torino}"));
    }
    if let Some(min_palermo) = query.min_palermo_scale {
        parts.push(format!("min_palermo_scale={min_palermo}"));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("&{}", parts.join("&"))
    }
}

/// Encodes the continuation cursor for the next page, from the last row of
/// this one — or `None` if `has_more` says there isn't a next page.
fn next_cursor(
    rows: &[AsteroidCatalogRecord],
    has_more: bool,
    sort: CatalogSortKey,
    sort_dir: SortDir,
) -> Option<String> {
    if !has_more {
        return None;
    }
    rows.last().map(|last| {
        let value = match sort {
            CatalogSortKey::ApproachActivity => CursorValue::Date(last.latest_approach_date),
            CatalogSortKey::Name => CursorValue::Text(last.name.clone()),
            CatalogSortKey::Diameter => CursorValue::Diameter(last.estimated_diameter_avg_km),
            CatalogSortKey::Torino => CursorValue::Torino(last.torino_scale),
            CatalogSortKey::Palermo => CursorValue::Palermo(last.palermo_scale),
        };
        CatalogCursor {
            sort,
            sort_dir,
            value,
            id: last.id,
        }
        .encode()
    })
}

/// GET /api/asteroids
///
/// Keyset-paginated asteroid catalog listing.
pub async fn catalog_list(
    State(state): State<AppState>,
    Query(query): Query<CatalogQuery>,
) -> Result<Json<ApiResponse<CursorPage<AsteroidCatalogRecord>>>, StatusCode> {
    let cursor = decode_cursor(&query)?;

    let (rows, has_more) = CatalogRepository::list(&state.db_pool, list_params(&query, cursor))
        .await
        .map_err(|e| {
            error!("Catalog listing query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let next_cursor = next_cursor(&rows, has_more, query.sort, query.sort_dir);

    Ok(Json(ApiResponse::success(CursorPage {
        data: rows,
        next_cursor,
    })))
}

/// GET /api/asteroids/{neo_reference_id}
///
/// Asteroid detail view: orbital elements, hazard scales, and approach history.
pub async fn catalog_detail(
    State(state): State<AppState>,
    Path(neo_reference_id): Path<String>,
) -> Result<Json<ApiResponse<AsteroidDetailRecord>>, StatusCode> {
    let detail = CatalogRepository::get_detail(&state.db_pool, &neo_reference_id)
        .await
        .map_err(|e| {
            error!("Catalog detail query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match detail {
        Some(detail) => Ok(Json(ApiResponse::success(detail))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// GET /api/asteroids/{neo_reference_id}/similar
///
/// Nearest neighbors by pgvector embedding distance (M5). 404 if the
/// asteroid doesn't exist or hasn't been vectorized yet (the `vectorize` CLI
/// job hasn't run for it).
pub async fn catalog_similar(
    State(state): State<AppState>,
    Path(neo_reference_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<AsteroidCatalogRecord>>>, StatusCode> {
    let similar = CatalogRepository::similar(&state.db_pool, &neo_reference_id, SIMILAR_LIMIT)
        .await
        .map_err(|e| {
            error!("Similar-asteroids query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match similar {
        Some(rows) => Ok(Json(ApiResponse::success(rows))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// GET /dashboard/catalog/{neo_reference_id}
///
/// SSR asteroid detail page.
pub async fn render_catalog_detail_page(
    State(state): State<AppState>,
    Path(neo_reference_id): Path<String>,
) -> Result<CatalogDetailTemplate, StatusCode> {
    let asteroid = CatalogRepository::get_detail(&state.db_pool, &neo_reference_id)
        .await
        .map_err(|e| {
            error!("Catalog detail page query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let similar = CatalogRepository::similar(&state.db_pool, &neo_reference_id, SIMILAR_LIMIT)
        .await
        .unwrap_or_else(|e| {
            error!("Similar-asteroids query failed for detail page: {}", e);
            None
        })
        .unwrap_or_default();

    Ok(CatalogDetailTemplate { asteroid, similar })
}

/// GET /dashboard/catalog
///
/// SSR catalog page, pre-populated with the first page of rows.
pub async fn render_catalog_page(State(state): State<AppState>) -> impl IntoResponse {
    let query = CatalogQuery::default();

    let (rows, has_more) =
        match CatalogRepository::list(&state.db_pool, list_params(&query, None)).await {
            Ok(result) => result,
            Err(e) => {
                error!("Catalog page query failed: {}", e);
                (Vec::new(), false)
            }
        };

    let next_cursor = next_cursor(&rows, has_more, query.sort, query.sort_dir);
    let sort = sort_key_str(query.sort).to_string();
    let sort_dir = sort_dir_str(query.sort_dir).to_string();
    let query_string = build_catalog_query_string(&query);

    let rows_html = CatalogRowsTemplate {
        rows,
        next_cursor: next_cursor.clone(),
        oob_next_button: false,
        sort: sort.clone(),
        sort_dir: sort_dir.clone(),
        query_string: query_string.clone(),
    }
    .render()
    .unwrap_or_else(|e| {
        error!("Failed to render initial catalog rows: {}", e);
        String::new()
    });

    CatalogTemplate {
        rows_html,
        next_cursor,
        sort,
        sort_dir,
        query_string,
    }
}

/// GET /dashboard/catalog/rows
///
/// HTMX partial: one page of catalog rows, driven by the pagination
/// footer's Prev/Next buttons (`?cursor=...`) or a filter-form change (no
/// cursor, i.e. back to page one).
pub async fn catalog_rows(
    State(state): State<AppState>,
    Query(query): Query<CatalogQuery>,
) -> Result<CatalogRowsTemplate, StatusCode> {
    let cursor = decode_cursor(&query)?;

    let (rows, has_more) = CatalogRepository::list(&state.db_pool, list_params(&query, cursor))
        .await
        .map_err(|e| {
            error!("Catalog rows query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let next_cursor = next_cursor(&rows, has_more, query.sort, query.sort_dir);

    Ok(CatalogRowsTemplate {
        rows,
        next_cursor,
        oob_next_button: true,
        sort: sort_key_str(query.sort).to_string(),
        sort_dir: sort_dir_str(query.sort_dir).to_string(),
        query_string: build_catalog_query_string(&query),
    })
}
