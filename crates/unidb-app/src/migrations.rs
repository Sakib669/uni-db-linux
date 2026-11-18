//! Migration files: load numbered SQL scripts, checksum them, and plan what is
//! pending against the applied ledger.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use unidb_core::{ErrorCategory, UniDbError};

/// Result of one migration run.
#[derive(Debug, Clone, Default)]
pub struct MigrationReport {
    pub applied: Vec<String>,
    pub skipped: usize,
    pub warnings: Vec<String>,
}

/// One discovered migration file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationFile {
    pub name: String,
    pub path: PathBuf,
    pub sql: String,
    pub checksum: String,
}

/// Loads `NNNN_name.sql` files from a directory in filename order.
pub fn load_migrations(dir: &Path) -> Result<Vec<MigrationFile>, UniDbError> {
    let entries = std::fs::read_dir(dir).map_err(|error| {
        UniDbError::new(
            "migration_dir_unreadable",
            ErrorCategory::Network,
            "The migrations directory could not be read",
        )
        .with_detail(error.to_string())
    })?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            UniDbError::internal("Could not read a migration directory entry")
                .with_detail(error.to_string())
        })?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("sql") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        let stem = name.trim_end_matches(".sql");
        let Some((number, label)) = stem.split_once('_') else {
            return Err(invalid_migration_name(&name));
        };
        if number.parse::<u32>().is_err() || label.is_empty() {
            return Err(invalid_migration_name(&name));
        }
        let sql = std::fs::read_to_string(&path).map_err(|error| {
            UniDbError::new(
                "migration_file_unreadable",
                ErrorCategory::Network,
                format!("The migration {name} could not be read"),
            )
            .with_detail(error.to_string())
        })?;
        files.push(MigrationFile {
            name,
            path,
            checksum: checksum(&sql),
            sql,
        });
    }
    files.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(files)
}

/// Pending files plus checksum warnings for already applied files.
pub fn plan_migrations(
    files: &[MigrationFile],
    applied: &[(String, String)],
) -> (Vec<MigrationFile>, Vec<String>) {
    let applied_names: HashSet<&str> = applied.iter().map(|(name, _)| name.as_str()).collect();
    let mut pending = Vec::new();
    let mut warnings = Vec::new();
    for file in files {
        match applied.iter().find(|(name, _)| name == &file.name) {
            Some((_, checksum)) if checksum != &file.checksum => {
                warnings.push(format!("{} changed since it was applied", file.name))
            }
            Some(_) => {}
            None => pending.push(file.clone()),
        }
    }
    let _ = applied_names;
    (pending, warnings)
}

/// Stable FNV-1a checksum in hex.
pub fn checksum(sql: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn invalid_migration_name(name: &str) -> UniDbError {
    UniDbError::new(
        "migration_name_invalid",
        ErrorCategory::Syntax,
        format!("{name} is not a numbered migration name"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_pending_files_and_warns_on_changed_ones() {
        let files = vec![
            MigrationFile {
                name: "0001_a.sql".to_string(),
                path: PathBuf::from("0001_a.sql"),
                sql: "SELECT 1".to_string(),
                checksum: "abc".to_string(),
            },
            MigrationFile {
                name: "0002_b.sql".to_string(),
                path: PathBuf::from("0002_b.sql"),
                sql: "SELECT 2".to_string(),
                checksum: "def".to_string(),
            },
        ];
        let applied = vec![("0001_a.sql".to_string(), "abc".to_string())];
        let (pending, warnings) = plan_migrations(&files, &applied);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].name, "0002_b.sql");
        assert!(warnings.is_empty());
        let changed = vec![("0001_a.sql".to_string(), "old".to_string())];
        let (_, warnings) = plan_migrations(&files, &changed);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn checksum_is_stable() {
        assert_eq!(checksum("SELECT 1"), checksum("SELECT 1"));
        assert_ne!(checksum("SELECT 1"), checksum("SELECT 2"));
    }
}
