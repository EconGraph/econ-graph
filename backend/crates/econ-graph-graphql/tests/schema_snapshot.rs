// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Fails when the GraphQL schema drifts from the committed `schema.graphql`.
//!
//! The frontend validates its operations against that file, so a schema change
//! has to land together with the regenerated snapshot. Regenerate it from
//! `backend/` with:
//!
//! ```sh
//! UPDATE_SCHEMA=1 cargo test -p econ-graph-graphql --test schema_snapshot
//! ```

use std::path::PathBuf;

/// Path of the committed snapshot: `schema.graphql` at the crate root.
fn snapshot_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema.graphql")
}

#[test]
fn schema_matches_committed_snapshot() {
    let printed = econ_graph_graphql::graphql::sdl();
    let path = snapshot_path();

    let update = std::env::var("UPDATE_SCHEMA").is_ok_and(|v| !v.is_empty() && v != "0");
    if update {
        std::fs::write(&path, &printed).expect("write schema.graphql");
        return;
    }

    // A CRLF checkout (core.autocrlf on Windows) is not drift.
    let committed = std::fs::read_to_string(&path)
        .map(|s| s.replace("\r\n", "\n"))
        .unwrap_or_else(|e| {
            panic!(
                "cannot read {}: {e}. Generate it with \
             `UPDATE_SCHEMA=1 cargo test -p econ-graph-graphql --test schema_snapshot`",
                path.display()
            )
        });

    if committed != printed {
        let committed_lines: Vec<&str> = committed.lines().collect();
        let printed_lines: Vec<&str> = printed.lines().collect();
        let first_diff = committed_lines
            .iter()
            .zip(&printed_lines)
            .position(|(a, b)| a != b)
            .unwrap_or(committed_lines.len().min(printed_lines.len()));
        let show = |line: Option<&&str>| match line {
            Some(line) => format!("{line:?}"),
            None => "<end of file>".to_string(),
        };
        panic!(
            "the GraphQL schema differs from {} (first difference at line {}).\n\
             Regenerate it from backend/ with \
             `UPDATE_SCHEMA=1 cargo test -p econ-graph-graphql --test schema_snapshot` \
             and commit the result.\n\
             committed: {}\n\
             printed:   {}",
            path.display(),
            first_diff + 1,
            show(committed_lines.get(first_diff)),
            show(printed_lines.get(first_diff)),
        );
    }
}
