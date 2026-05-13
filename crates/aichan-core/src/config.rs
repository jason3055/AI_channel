use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{io_error, json_error, Result};
use crate::state::LocalStateDir;

pub const DEFAULT_BASE_URL: &str = "https://aichan-server-474569752665.us-central1.run.app";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AichanConfig {
    pub base_url: Option<String>,
}

impl AichanConfig {
    pub fn load_or_default(state: &LocalStateDir) -> Result<Self> {
        let path = state.config_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        Self::read_from(path)
    }

    pub fn effective_base_url<'a>(&'a self, override_url: Option<&'a str>) -> &'a str {
        override_url
            .or(self.base_url.as_deref())
            .unwrap_or(DEFAULT_BASE_URL)
    }

    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
        serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))
    }

    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(path, source))?;
        std::fs::write(path, bytes).map_err(|source| io_error(path, source))
    }
}
