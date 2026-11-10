//! A role is in the catalog only once something checks it (see the `roles` module doc).
//!
//! Catalog reads are checked through `Role::for_catalog`, which maps a data source to its role.
//! Every other role must be named as `Role::<Variant>` somewhere in the backend's code outside
//! the catalog itself, or an operator granting it would believe it does something.

use std::fs;
use std::path::{Path, PathBuf};

use econ_graph_auth::Role;

/// Every `.rs` file under `dir`, skipping build output.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("readable source directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn every_non_catalog_role_is_checked_somewhere() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend/crates")
        .to_path_buf();
    let catalog = crates.join("econ-graph-auth/src/roles.rs");
    let mut files = Vec::new();
    rust_files(&crates, &mut files);
    let this_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/role_catalog_enforced.rs");
    assert!(this_file.is_file(), "{}", this_file.display());
    let sources: Vec<String> = files
        .iter()
        .filter(|path| **path != catalog && **path != this_file)
        .map(|path| fs::read_to_string(path).expect("readable source file"))
        .collect();
    assert!(
        sources.len() > 50,
        "found only {} source files",
        sources.len()
    );

    let unchecked: Vec<&str> = Role::all()
        .iter()
        .filter(|role| !role.is_catalog())
        .filter(|role| {
            let reference = format!("Role::{role:?}");
            !sources.iter().any(|source| {
                source.match_indices(&reference).any(|(at, _)| {
                    // `Role::AdminUsersRead` must not count as `Role::AdminUsers`.
                    !source[at + reference.len()..]
                        .starts_with(|c: char| c.is_alphanumeric() || c == '_')
                })
            })
        })
        .map(|role| role.as_str())
        .collect();
    assert!(
        unchecked.is_empty(),
        "these catalog roles are checked nowhere; remove them or enforce them: {unchecked:?}"
    );
}
