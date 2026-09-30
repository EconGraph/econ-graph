// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Turns a generated `backend/flags/<profile>.json` (see `config/flags/README.md`) into
//! `cfg(flag_<key>)` directives for a crate's `build.rs`.
//!
//! Only `"kind": "build"` flags become cfgs: `preview`, `ops` and `experiment` flags are
//! runtime concerns, not something a binary is compiled with or without.

use std::collections::BTreeSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// The environment variable selecting a flag profile. Unset means `release`; any other value
/// must be one of [`PROFILES`], or [`plan`] refuses it with [`Error::UnknownProfile`].
pub const PROFILE_ENV: &str = "FLAGS_PROFILE";

/// The flag profiles a generated flags directory holds a file for.
pub const PROFILES: [&str; 2] = ["release", "dev"];

/// Why a flags directory could not be turned into a [`Plan`].
#[derive(Debug)]
pub enum Error {
    /// `FLAGS_PROFILE` was set to something other than one of [`PROFILES`].
    UnknownProfile(String),
    /// No `flags` directory was found above `CARGO_MANIFEST_DIR`.
    FlagsDirNotFound(PathBuf),
    /// The profile's JSON file could not be read.
    Read(PathBuf, std::io::Error),
    /// The profile's JSON file could not be parsed.
    Parse(PathBuf, serde_json::Error),
    /// The file's own `profile` field does not match the file name it was read from.
    WrongProfile {
        path: PathBuf,
        found: String,
        expected: String,
    },
    /// A flag key is not a valid Rust identifier fragment once prefixed with `flag_`.
    BadKey(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProfile(value) => {
                write!(f, "{PROFILE_ENV}={value:?} is not one of {PROFILES:?}")
            }
            Self::FlagsDirNotFound(start) => {
                write!(f, "no `flags` directory found above {}", start.display())
            }
            Self::Read(path, why) => write!(f, "could not read {}: {why}", path.display()),
            Self::Parse(path, why) => write!(f, "could not parse {}: {why}", path.display()),
            Self::WrongProfile {
                path,
                found,
                expected,
            } => write!(
                f,
                "{} says profile {found:?}, expected {expected:?}",
                path.display()
            ),
            Self::BadKey(key) => write!(f, "flag key {key:?} is not a valid cfg identifier"),
        }
    }
}

impl std::error::Error for Error {}

/// What a profile's flags file resolves to: which `build` flags are on, every `build` flag
/// this profile knows about (on or off), and the file(s) read.
pub struct Plan {
    pub enabled: BTreeSet<String>,
    pub known: BTreeSet<String>,
    pub files: Vec<PathBuf>,
}

impl Plan {
    /// `cargo::` directives for `build.rs`: a `rustc-cfg=flag_<key>` for each enabled flag, and
    /// a `rustc-check-cfg` for every known flag so `cfg(flag_<key>)` never warns as unexpected.
    pub fn directives(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .known
            .iter()
            .map(|key| format!("cargo::rustc-check-cfg=cfg(flag_{key})"))
            .collect();
        lines.extend(
            self.enabled
                .iter()
                .map(|key| format!("cargo::rustc-cfg=flag_{key}")),
        );
        lines
    }
}

#[derive(serde::Deserialize)]
struct FlagsFile {
    profile: String,
    flags: std::collections::BTreeMap<String, FlagEntry>,
}

#[derive(serde::Deserialize)]
struct FlagEntry {
    kind: String,
    value: bool,
}

fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !key.starts_with(|c: char| c.is_ascii_digit())
}

/// Resolve `flags_dir/<profile>.json` (`profile` from `FLAGS_PROFILE`, via `profile_env`, or
/// `"release"` when unset) into a [`Plan`].
pub fn plan(flags_dir: &Path, profile: Option<&str>) -> Result<Plan, Error> {
    let profile = profile.unwrap_or("release");
    if !PROFILES.contains(&profile) {
        return Err(Error::UnknownProfile(profile.to_string()));
    }

    let path = flags_dir.join(format!("{profile}.json"));
    let contents = fs::read_to_string(&path).map_err(|e| Error::Read(path.clone(), e))?;
    let parsed: FlagsFile =
        serde_json::from_str(&contents).map_err(|e| Error::Parse(path.clone(), e))?;

    if parsed.profile != profile {
        return Err(Error::WrongProfile {
            path,
            found: parsed.profile,
            expected: profile.to_string(),
        });
    }

    let mut known = BTreeSet::new();
    let mut enabled = BTreeSet::new();
    for (key, entry) in parsed.flags {
        if entry.kind != "build" {
            continue;
        }
        if !is_valid_key(&key) {
            return Err(Error::BadKey(key));
        }
        known.insert(key.clone());
        if entry.value {
            enabled.insert(key);
        }
    }

    Ok(Plan {
        enabled,
        known,
        files: vec![path],
    })
}

/// Walk up from `start` looking for a `flags` directory (as `backend/flags` sits next to
/// `backend/Cargo.toml`), so this works the same whether `build.rs` runs from the workspace
/// root or a crate directory.
pub fn find_flags_dir(start: &Path) -> Result<PathBuf, Error> {
    let mut dir = start;
    loop {
        let candidate = dir.join("flags");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return Err(Error::FlagsDirNotFound(start.to_path_buf())),
        }
    }
}

/// Called from a crate's `build.rs`: reads `FLAGS_PROFILE`, finds `backend/flags`, and emits
/// the `cargo::` directives that turn each on `build` flag into `cfg(flag_<key>)`. Panics on
/// any error, since a build that can't tell what it's building shouldn't silently proceed.
pub fn emit() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let profile = env::var(PROFILE_ENV).ok();

    println!("cargo::rerun-if-env-changed={PROFILE_ENV}");

    let flags_dir = find_flags_dir(Path::new(&manifest_dir)).unwrap_or_else(|e| panic!("{e}"));
    let plan = plan(&flags_dir, profile.as_deref()).unwrap_or_else(|e| panic!("{e}"));

    for file in &plan.files {
        println!("cargo::rerun-if-changed={}", file.display());
    }
    for directive in plan.directives() {
        println!("{directive}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_flags(dir: &Path, profile: &str, flags: &str) {
        fs::write(
            dir.join(format!("{profile}.json")),
            format!(r#"{{"profile": "{profile}", "flags": {flags}}}"#),
        )
        .unwrap();
    }

    #[test]
    fn unset_profile_is_release() {
        let dir = tempfile::tempdir().unwrap();
        write_flags(
            dir.path(),
            "release",
            r#"{"mcp": {"kind": "build", "value": false}}"#,
        );
        let plan = plan(dir.path(), None).unwrap();
        assert!(plan.enabled.is_empty());
        assert_eq!(plan.known, BTreeSet::from(["mcp".to_string()]));
    }

    #[test]
    fn dev_profile_turns_on_dev_flags() {
        let dir = tempfile::tempdir().unwrap();
        write_flags(
            dir.path(),
            "dev",
            r#"{"mcp": {"kind": "build", "value": true}}"#,
        );
        let plan = plan(dir.path(), Some("dev")).unwrap();
        assert_eq!(plan.enabled, BTreeSet::from(["mcp".to_string()]));
        assert_eq!(
            plan.directives(),
            vec![
                "cargo::rustc-check-cfg=cfg(flag_mcp)".to_string(),
                "cargo::rustc-cfg=flag_mcp".to_string(),
            ]
        );
    }

    #[test]
    fn only_build_flags_become_cfgs() {
        let dir = tempfile::tempdir().unwrap();
        write_flags(
            dir.path(),
            "release",
            r#"{"mcp": {"kind": "build", "value": true}, "some_ops_switch": {"kind": "ops", "value": true}}"#,
        );
        let plan = plan(dir.path(), Some("release")).unwrap();
        assert_eq!(plan.enabled, BTreeSet::from(["mcp".to_string()]));
        assert_eq!(plan.known, BTreeSet::from(["mcp".to_string()]));
    }

    #[test]
    fn bad_input_is_refused() {
        let dir = tempfile::tempdir().unwrap();

        assert!(matches!(
            plan(dir.path(), Some("staging")),
            Err(Error::UnknownProfile(p)) if p == "staging"
        ));

        write_flags(
            dir.path(),
            "release",
            r#"{"mcp": {"kind": "build", "value": true}}"#,
        );
        assert!(matches!(
            plan(dir.path(), Some("dev")),
            Err(Error::Read(..))
        ));

        fs::write(
            dir.path().join("dev.json"),
            r#"{"profile": "release", "flags": {}}"#,
        )
        .unwrap();
        assert!(matches!(
            plan(dir.path(), Some("dev")),
            Err(Error::WrongProfile { .. })
        ));

        fs::write(dir.path().join("dev.json"), "not json").unwrap();
        assert!(matches!(
            plan(dir.path(), Some("dev")),
            Err(Error::Parse(..))
        ));

        write_flags(
            dir.path(),
            "dev",
            r#"{"not-a-cfg-name": {"kind": "build", "value": true}}"#,
        );
        assert!(matches!(
            plan(dir.path(), Some("dev")),
            Err(Error::BadKey(_))
        ));
    }

    #[test]
    fn finds_the_nearest_flags_dir() {
        let dir = tempfile::tempdir().unwrap();
        let flags = dir.path().join("flags");
        fs::create_dir(&flags).unwrap();
        let nested = dir.path().join("backend/crates/some-crate");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(find_flags_dir(&nested).unwrap(), flags);
    }

    /// The generated files actually committed at `backend/flags/` parse under this crate's
    /// rules, so a change to their shape breaks this test before it breaks a real build.
    #[test]
    fn committed_files_parse() {
        let flags_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flags");
        for profile in PROFILES {
            let plan = plan(&flags_dir, Some(profile)).unwrap_or_else(|e| panic!("{profile}: {e}"));
            assert!(plan.known.contains("mcp"), "{profile}: {:?}", plan.known);
        }
    }
}
