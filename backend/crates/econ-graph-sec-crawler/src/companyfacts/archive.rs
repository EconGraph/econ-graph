use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use anyhow::{bail, Context, Result};
use zip::ZipArchive;

/// Hard operational bounds, independent of untrusted ZIP metadata.
#[derive(Debug, Clone, Copy)]
pub struct ArchiveLimits {
    pub max_download_bytes: u64,
    pub max_entry_bytes: u64,
    pub max_uncompressed_bytes: u64,
    pub max_entries: usize,
}
impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_download_bytes: 16 * 1024 * 1024 * 1024,
            max_entry_bytes: 128 * 1024 * 1024,
            max_uncompressed_bytes: 128 * 1024 * 1024 * 1024,
            max_entries: 1_000_000,
        }
    }
}

pub struct BulkArchive {
    archive: ZipArchive<File>,
    pub entries: BTreeMap<String, usize>,
    max_entry_bytes: u64,
}
impl BulkArchive {
    /// Check the complete central directory AND every payload CRC before any database write.
    /// No entry is extracted: names are identifiers, never paths on the filesystem.
    pub fn open_verified(path: &Path, limits: ArchiveLimits) -> Result<Self> {
        let file = File::open(path)?;
        if file.metadata()?.len() > limits.max_download_bytes {
            bail!("bulk archive exceeds download limit");
        }
        let mut archive = ZipArchive::new(file).context("incomplete or invalid bulk ZIP")?;
        if archive.len() > limits.max_entries || archive.is_empty() {
            bail!("invalid bulk entry count");
        }
        let mut entries = BTreeMap::new();
        let mut total = 0u64;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name()?.into_owned();
            if entry.is_dir() || !valid_name(&name) {
                bail!("unexpected bulk archive entry {name:?}");
            }
            if entries.insert(name.clone(), i).is_some() {
                bail!("duplicate bulk archive entry {name}");
            }
            if entry.size() > limits.max_entry_bytes {
                bail!("bulk JSON entry exceeds limit: {name}");
            }
            let count = io::copy(
                &mut entry.by_ref().take(limits.max_entry_bytes + 1),
                &mut io::sink(),
            )?;
            if count > limits.max_entry_bytes {
                bail!("bulk JSON entry exceeds limit: {name}");
            }
            total = total
                .checked_add(count)
                .context("uncompressed byte overflow")?;
            if total > limits.max_uncompressed_bytes {
                bail!("uncompressed bulk archive exceeds limit");
            }
        }
        Ok(Self {
            archive,
            entries,
            max_entry_bytes: limits.max_entry_bytes,
        })
    }
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let index = *self
            .entries
            .get(name)
            .with_context(|| format!("missing bulk entry {name}"))?;
        let entry = self.archive.by_index(index)?;
        let mut bytes = Vec::new();
        entry
            .take(self.max_entry_bytes + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > self.max_entry_bytes {
            bail!("bulk JSON entry exceeds limit");
        }
        Ok(bytes)
    }
    pub fn company_names(&self) -> Vec<String> {
        self.entries
            .keys()
            .filter(|n| n.len() == 18)
            .cloned()
            .collect()
    }
}
fn valid_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("CIK") else {
        return false;
    };
    if rest.len() < 15 || !rest.as_bytes()[..10].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let suffix = &rest[10..];
    suffix == ".json"
        || suffix
            .strip_prefix("-submissions-")
            .and_then(|s| s.strip_suffix(".json"))
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}

pub fn ensure_unique_historical_files(names: &[String], cik: &str) -> Result<()> {
    let mut seen = HashSet::new();
    for name in names {
        if !valid_name(name)
            || !name.starts_with(&format!("CIK{cik}-submissions-"))
            || !seen.insert(name)
        {
            bail!("invalid/duplicate historical submissions filename");
        }
    }
    Ok(())
}
