// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Stable series ids.
//!
//! A series' id is derived from its natural key, never generated: UUIDv5 of
//! `"{SOURCE}:{external_id}"` (for example `FRED:GDP`) in [`SERIES_ID_NAMESPACE`], where `SOURCE`
//! is [`SourceId::as_str`] and `external_id` is the value stored in `external_id` (at most 255
//! characters). [`persist`](crate::persist) uses it for every `economic_series` row it creates and
//! every `series_metadata` row it writes, so a reload or a fresh database gives every series the
//! same id, and anything that refers to a series by id (notes, saved charts, links) keeps working.
//!
//! In a database from before stable ids, `economic_series` rows keep their generated ids (data
//! points and crawl attempts reference them); only `series_metadata` rows move onto stable ids,
//! when rediscovered. Recreate such a database to get stable ids everywhere.
//!
//! The id depends on the [`SourceId`], not on the `data_sources` row. If a second `data_sources`
//! row ever existed for one source (for example after renaming the seeded row), creating a series
//! under it fails with a primary key violation instead of silently duplicating the series.
//!
//! Changing the namespace or the name format changes every id: don't.

use uuid::Uuid;

use crate::source::SourceId;

/// Namespace for [`stable_series_id`]: UUIDv5 of `https://econgraph.com/ns/series` in the
/// standard URL namespace. Fixed forever.
pub const SERIES_ID_NAMESPACE: Uuid = Uuid::from_u128(0x1f871bd6_25e5_5022_b7ba_790efbc3bd30);

/// The stable id of the series `external_id` of `source`. See the [module docs](self).
pub fn stable_series_id(source: SourceId, external_id: &str) -> Uuid {
    Uuid::new_v5(
        &SERIES_ID_NAMESPACE,
        format!("{}:{external_id}", source.as_str()).as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_is_derived_from_the_econgraph_url() {
        assert_eq!(
            SERIES_ID_NAMESPACE,
            Uuid::new_v5(&Uuid::NAMESPACE_URL, b"https://econgraph.com/ns/series")
        );
    }

    /// These ids are referenced from outside the database (notes, links). If this test fails,
    /// the change breaks every stored reference: revert it.
    #[test]
    fn known_ids_are_pinned() {
        for (source, external_id, id) in [
            (
                SourceId::Fred,
                "GDP",
                "d8124fe6-ef1c-52dd-8d22-c1976625064c",
            ),
            (
                SourceId::Fred,
                "UNRATE",
                "d2ac622b-7d71-5884-9994-0f939bd9407f",
            ),
            (
                SourceId::Bls,
                "CES0000000001",
                "201c52b2-7f26-533b-a4d6-54e3141ee87b",
            ),
            (
                SourceId::Census,
                "CENSUS_BDS_ESTAB_us",
                "d5c534fc-0ebc-58de-8d28-dccb1ba66847",
            ),
        ] {
            assert_eq!(
                stable_series_id(source, external_id).to_string(),
                id,
                "{source}:{external_id}"
            );
        }
    }

    #[test]
    fn source_is_part_of_the_key() {
        // BEA also has a series called GDP.
        assert_ne!(
            stable_series_id(SourceId::Fred, "GDP"),
            stable_series_id(SourceId::Bea, "GDP")
        );
        assert_eq!(stable_series_id(SourceId::Fred, "GDP").get_version_num(), 5);
    }
}
