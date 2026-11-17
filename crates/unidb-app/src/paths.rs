//! Where the app keeps its own data.

use std::path::PathBuf;

/// `$UNIDB_DATA_DIR`, else `$XDG_DATA_HOME/unidb`, else `~/.local/share/unidb`.
pub fn default_data_dir() -> PathBuf {
    if let Some(directory) = std::env::var_os("UNIDB_DATA_DIR") {
        return PathBuf::from(directory);
    }
    if let Some(directory) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(directory).join("unidb");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/unidb");
    }
    PathBuf::from("unidb-data")
}
