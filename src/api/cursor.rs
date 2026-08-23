//! Opaque keyset-pagination cursor for the asteroid catalog listing
//! (`GET /asteroids`).
//!
//! The catalog can be sorted by any [`CatalogSortKey`] in either
//! [`SortDir`]. A cursor carries the active sort key/direction plus the
//! corresponding column value of the last row on the previous page, tied by
//! `id`, base64-encoded so callers treat it as an opaque token rather than
//! constructing `WHERE` values themselves. Nullable sort columns carry their
//! value explicitly (rather than relying on SQL's `NULLS LAST` alone)
//! because a plain `WHERE (col, id) < ($1, $2)` comparison doesn't correctly
//! re-enter the `NULL` tail of the ordering — the query has to branch on
//! whether the cursor itself was in the `NULL` tier.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::api::types::{CatalogSortKey, SortDir};

/// The sort key of the last row returned on the previous page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogCursor {
    /// Which column this cursor was built for; a cursor from a different
    /// sort than the incoming request's is meaningless, so callers should
    /// treat a mismatch the same as a malformed cursor.
    pub sort: CatalogSortKey,
    /// Direction the cursor was built for.
    pub sort_dir: SortDir,
    /// The sort column's value on the last row of the previous page.
    pub value: CursorValue,
    /// Tie-breaker for rows sharing the same sort value.
    pub id: Uuid,
}

/// The sort column's value carried by a [`CatalogCursor`], typed per
/// [`CatalogSortKey`] variant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CursorValue {
    /// `MAX(close_approach_date)` across the asteroid's approaches, or
    /// `None` if it has none. Used by [`CatalogSortKey::ApproachActivity`].
    Date(Option<NaiveDate>),
    /// Asteroid name. Used by [`CatalogSortKey::Name`].
    Text(String),
    /// Average estimated diameter, in kilometers. Used by
    /// [`CatalogSortKey::Diameter`].
    Diameter(f64),
    /// Torino Scale. Used by [`CatalogSortKey::Torino`].
    Torino(Option<i16>),
    /// Cumulative Palermo Scale. Used by [`CatalogSortKey::Palermo`].
    Palermo(Option<f64>),
}

/// A pagination cursor that failed to decode.
#[derive(Debug, Error)]
#[error("malformed pagination cursor")]
pub struct CursorError;

impl CatalogCursor {
    /// Encodes this cursor as an opaque, URL-safe token.
    pub fn encode(&self) -> String {
        let json = serde_json::to_vec(self).expect("CatalogCursor always serializes");
        URL_SAFE_NO_PAD.encode(json)
    }

    /// Decodes a token previously produced by [`CatalogCursor::encode`].
    ///
    /// # Errors
    ///
    /// Returns [`CursorError`] if `token` isn't valid base64, or doesn't
    /// deserialize into a `CatalogCursor` — treated identically (a malformed
    /// client-supplied cursor either way), rather than leaking which stage failed.
    pub fn decode(token: &str) -> Result<Self, CursorError> {
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| CursorError)?;
        serde_json::from_slice(&bytes).map_err(|_| CursorError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_with_date() {
        let cursor = CatalogCursor {
            sort: CatalogSortKey::ApproachActivity,
            sort_dir: SortDir::Desc,
            value: CursorValue::Date(NaiveDate::from_ymd_opt(2026, 3, 5)),
            id: Uuid::new_v4(),
        };

        let decoded = CatalogCursor::decode(&cursor.encode()).unwrap();

        assert_eq!(decoded, cursor);
    }

    #[test]
    fn test_roundtrip_without_date() {
        let cursor = CatalogCursor {
            sort: CatalogSortKey::ApproachActivity,
            sort_dir: SortDir::Desc,
            value: CursorValue::Date(None),
            id: Uuid::new_v4(),
        };

        let decoded = CatalogCursor::decode(&cursor.encode()).unwrap();

        assert_eq!(decoded, cursor);
    }

    #[test]
    fn test_roundtrip_name_asc() {
        let cursor = CatalogCursor {
            sort: CatalogSortKey::Name,
            sort_dir: SortDir::Asc,
            value: CursorValue::Text("Apophis".to_string()),
            id: Uuid::new_v4(),
        };

        let decoded = CatalogCursor::decode(&cursor.encode()).unwrap();

        assert_eq!(decoded, cursor);
    }

    #[test]
    fn test_roundtrip_diameter() {
        let cursor = CatalogCursor {
            sort: CatalogSortKey::Diameter,
            sort_dir: SortDir::Desc,
            value: CursorValue::Diameter(1.234),
            id: Uuid::new_v4(),
        };

        let decoded = CatalogCursor::decode(&cursor.encode()).unwrap();

        assert_eq!(decoded, cursor);
    }

    #[test]
    fn test_roundtrip_torino_and_palermo() {
        let cursor = CatalogCursor {
            sort: CatalogSortKey::Torino,
            sort_dir: SortDir::Asc,
            value: CursorValue::Torino(Some(3)),
            id: Uuid::new_v4(),
        };
        assert_eq!(CatalogCursor::decode(&cursor.encode()).unwrap(), cursor);

        let cursor = CatalogCursor {
            sort: CatalogSortKey::Palermo,
            sort_dir: SortDir::Desc,
            value: CursorValue::Palermo(None),
            id: Uuid::new_v4(),
        };
        assert_eq!(CatalogCursor::decode(&cursor.encode()).unwrap(), cursor);
    }

    #[test]
    fn test_decode_rejects_garbage() {
        assert!(CatalogCursor::decode("not a cursor").is_err());
        assert!(CatalogCursor::decode("").is_err());
    }

    #[test]
    fn test_decode_rejects_valid_base64_wrong_shape() {
        let unrelated = URL_SAFE_NO_PAD.encode(b"\"just a string\"");
        assert!(CatalogCursor::decode(&unrelated).is_err());
    }
}
