pub mod chunking;
pub mod engine;
pub mod imports;
pub mod languages;
pub mod symbols;
pub mod types;

pub use engine::ParserEngine;
pub use types::{ParsedFile, CodeSymbol, ImportStatement, CodeChunk};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ParseReport {
    pub total_files: usize,
    pub total_symbols: usize,
    pub total_imports: usize,
    pub total_chunks: usize,
    pub files: Vec<ParsedFile>,
}
