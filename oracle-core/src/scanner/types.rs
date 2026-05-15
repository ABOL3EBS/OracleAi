use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryFile {
    pub path: String,
    pub size: u64,
    pub extension: Option<String>,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryScanResult {
    pub root_path: String,
    pub total_files: usize,
    pub total_size: u64,
    pub languages: HashMap<String, usize>,
    pub files: Vec<RepositoryFile>,
    pub scan_duration_ms: u128,
}
