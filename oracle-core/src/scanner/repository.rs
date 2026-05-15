use crate::scanner::language::detect_language;
use crate::scanner::types::{RepositoryFile, RepositoryScanResult};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;

pub struct RepoScanner {
    ignore_dirs: Vec<String>,
}

impl RepoScanner {
    pub fn new() -> Self {
        Self {
            ignore_dirs: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "dist".to_string(),
                "build".to_string(),
                "target".to_string(),
                ".next".to_string(),
                "__pycache__".to_string(),
            ],
        }
    }

    pub fn scan<P: AsRef<Path>>(&self, path: P) -> Result<RepositoryScanResult> {
        let start_time = Instant::now();
        let root = path.as_ref();

        let entries: Vec<_> = WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| {
                let file_name = e.file_name().to_string_lossy();
                !self.ignore_dirs.contains(&file_name.into_owned())
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .collect();

        let files: Vec<RepositoryFile> = entries
            .par_iter()
            .map(|entry| {
                let path = entry.path();
                let metadata = entry.metadata().ok();
                let size = metadata.map(|m| m.len()).unwrap_or(0);
                let extension = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_string());
                
                let language = extension
                    .as_ref()
                    .map(|ext| detect_language(ext))
                    .unwrap_or_else(|| "Unknown".to_string());

                RepositoryFile {
                    path: path.strip_prefix(root)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_string(),
                    size,
                    extension,
                    language,
                }
            })
            .collect();

        let mut languages = HashMap::new();
        let mut total_size = 0;

        for file in &files {
            *languages.entry(file.language.clone()).or_insert(0) += 1;
            total_size += file.size;
        }

        let scan_duration_ms = start_time.elapsed().as_millis();

        Ok(RepositoryScanResult {
            root_path: root.to_string_lossy().to_string(),
            total_files: files.len(),
            total_size,
            languages,
            files,
            scan_duration_ms,
        })
    }
}
