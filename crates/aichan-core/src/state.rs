use std::path::{Path, PathBuf};

use crate::error::{io_error, Result};

#[derive(Debug, Clone)]
pub struct LocalStateDir {
    project_root: PathBuf,
}

impl LocalStateDir {
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        Self {
            project_root: project_root.as_ref().to_path_buf(),
        }
    }

    pub fn root(&self) -> PathBuf {
        self.project_root.join(".aichan")
    }

    pub fn identity_path(&self) -> PathBuf {
        self.root().join("identity.json")
    }

    pub fn device_path(&self) -> PathBuf {
        self.root().join("device.json")
    }

    pub fn memory_path(&self) -> PathBuf {
        self.root().join("memory.json")
    }

    pub fn config_path(&self) -> PathBuf {
        self.root().join("config.json")
    }

    pub fn backup_metadata_path(&self) -> PathBuf {
        self.root().join("backup.json")
    }

    pub fn inbox_cache_dir(&self) -> PathBuf {
        self.root().join("inbox-cache")
    }

    pub fn transcripts_dir(&self) -> PathBuf {
        self.root().join("transcripts")
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.inbox_cache_dir())
            .map_err(|source| io_error(self.inbox_cache_dir(), source))
    }
}
