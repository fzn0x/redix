use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use anyhow::Result;

pub struct PersistenceManager {
    log_path: PathBuf,
    snapshot_path: PathBuf,
    metadata_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct RaftMetadata {
    pub current_term: u64,
    pub voted_for: Option<u64>,
}

impl PersistenceManager {
    pub fn new(node_id: u64) -> Self {
        let log_path = PathBuf::from(format!("node_{}.log", node_id));
        let snapshot_path = PathBuf::from(format!("node_{}.rdb", node_id));
        let metadata_path = PathBuf::from(format!("node_{}.meta", node_id));
        Self { log_path, snapshot_path, metadata_path }
    }

    pub fn save_metadata(&self, meta: &RaftMetadata) -> Result<()> {
        let data = bincode::serialize(meta)?;
        let mut file = File::create(&self.metadata_path)?;
        file.write_all(&data)?;
        file.sync_all()?;
        Ok(())
    }

    pub fn load_metadata(&self) -> Result<Option<RaftMetadata>> {
        if !self.metadata_path.exists() {
            return Ok(None);
        }
        let mut file = File::open(&self.metadata_path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        let meta = bincode::deserialize(&data)?;
        Ok(Some(meta))
    }

    /// Appends a single entry to the AOF log file.
    pub fn append_log<T: Serialize>(&self, entry: &T) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        
        let data = bincode::serialize(entry)?;
        let len = data.len() as u32;
        file.write_all(&len.to_le_bytes())?;
        file.write_all(&data)?;
        file.sync_all()?;
        Ok(())
    }

    /// Reads all entries from the AOF log file.
    pub fn read_log<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        if !self.log_path.exists() {
            return Ok(vec![]);
        }
        let mut file = File::open(&self.log_path)?;
        let mut entries = Vec::new();
        let mut len_buf = [0u8; 4];
        
        while file.read_exact(&mut len_buf).is_ok() {
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            file.read_exact(&mut data)?;
            let entry = bincode::deserialize(&data)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Truncates the AOF log and writes all provided entries (used after snapshotting).
    pub fn rewrite_log<T: Serialize>(&self, entries: &[T]) -> Result<()> {
        let mut file = File::create(&self.log_path)?;
        for entry in entries {
            let data = bincode::serialize(entry)?;
            let len = data.len() as u32;
            file.write_all(&len.to_le_bytes())?;
            file.write_all(&data)?;
        }
        file.sync_all()?;
        Ok(())
    }

    /// Saves a snapshot (RDB) of the state machine.
    pub fn save_snapshot<T: Serialize>(&self, state: &T) -> Result<()> {
        let data = bincode::serialize(state)?;
        let mut file = File::create(&self.snapshot_path)?;
        file.write_all(&data)?;
        file.sync_all()?;
        Ok(())
    }

    /// Loads the latest snapshot (RDB) of the state machine.
    pub fn load_snapshot<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        if !self.snapshot_path.exists() {
            return Ok(None);
        }
        let mut file = File::open(&self.snapshot_path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        let state = bincode::deserialize(&data)?;
        Ok(Some(state))
    }
}
