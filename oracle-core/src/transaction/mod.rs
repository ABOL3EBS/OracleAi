use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use chrono::{DateTime, Utc};
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TransactionStatus {
    Planned,
    Applied,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefactorTransaction {
    pub transaction_id: String,
    pub affected_files: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub status: TransactionStatus,
}

impl RefactorTransaction {
    pub fn new(id: String, affected_files: Vec<String>) -> Self {
        Self {
            transaction_id: id,
            affected_files,
            timestamp: Utc::now(),
            status: TransactionStatus::Planned,
        }
    }

    pub fn write_journal(&self) -> Result<()> {
        let dir = Path::new("transactions");
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        
        let file_path = dir.join(format!("{}.json", self.transaction_id));
        let content = serde_json::to_string_pretty(self)?;
        fs::write(file_path, content)?;
        Ok(())
    }
}
