use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{io_error, json_error, AichanError, Result};
use crate::state::LocalStateDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new() -> Self {
        Self(format!("device_{}", Uuid::new_v4().simple()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<()> {
        let Some(suffix) = self.0.strip_prefix("device_") else {
            return Err(AichanError::InvalidDevice(
                "device_id must start with device_".to_string(),
            ));
        };
        if suffix.len() != 32 || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(AichanError::InvalidDevice(
                "device_id must be device_ followed by 32 hex characters".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFile {
    pub version: u8,
    pub device_id: DeviceId,
    pub created_at: DateTime<Utc>,
}

impl DeviceFile {
    pub fn create_or_load(state: &LocalStateDir) -> Result<Self> {
        let path = state.device_path();
        if path.exists() {
            return Self::read_from(&path);
        }

        state.ensure_dirs()?;
        let device = Self {
            version: 1,
            device_id: DeviceId::new(),
            created_at: Utc::now(),
        };
        device.write_to(path)?;
        Ok(device)
    }

    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
        let device: Self =
            serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))?;
        device.validate()?;
        Ok(device)
    }

    fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(path, source))?;
        std::fs::write(path, bytes).map_err(|source| io_error(path, source))
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(AichanError::InvalidDevice(format!(
                "unsupported version {}",
                self.version
            )));
        }
        self.device_id.validate()
    }
}
