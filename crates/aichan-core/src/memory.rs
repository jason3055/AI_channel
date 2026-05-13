use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{io_error, json_error, AichanError, Result};
use crate::state::LocalStateDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryFile {
    pub version: u8,
    pub profile: AgentProfile,
    pub common_tags: Vec<String>,
    pub discovered_peers: Vec<PeerSummary>,
    pub interactions: Vec<InteractionSummary>,
    pub sync: SyncState,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentProfile {
    pub nickname: Option<String>,
    pub self_description: Option<String>,
    pub preferences: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSummary {
    pub peer_id: String,
    pub tags: Vec<String>,
    pub body_preview: Option<String>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionSummary {
    pub peer_id: String,
    pub summary: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SyncState {
    pub last_sync_at: Option<DateTime<Utc>>,
    pub inbox_cursor: Option<String>,
    pub activity_cursor: Option<String>,
}

impl Default for MemoryFile {
    fn default() -> Self {
        Self {
            version: 1,
            profile: AgentProfile::default(),
            common_tags: Vec::new(),
            discovered_peers: Vec::new(),
            interactions: Vec::new(),
            sync: SyncState::default(),
            updated_at: Utc::now(),
        }
    }
}

impl MemoryFile {
    pub fn create_or_load(state: &LocalStateDir) -> Result<Self> {
        let path = state.memory_path();
        if path.exists() {
            return Self::read_from(&path);
        }

        state.ensure_dirs()?;
        let memory = Self::default();
        memory.write_to(path)?;
        Ok(memory)
    }

    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
        let memory: Self =
            serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))?;
        memory.validate()?;
        Ok(memory)
    }

    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(path, source))?;
        std::fs::write(path, bytes).map_err(|source| io_error(path, source))
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(AichanError::InvalidMemory(format!(
                "unsupported version {}",
                self.version
            )));
        }
        Ok(())
    }
}
