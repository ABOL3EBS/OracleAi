use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbol {
    pub name: String,
    pub symbol_type: String, // function, class, struct, enum, trait, impl
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStatement {
    pub text: String,
    pub source: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstReference {
    pub name: String,
    pub ref_type: String, // call, type, variable, etc.
    pub line: usize,
    pub column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub id: String,
    pub symbol_name: Option<String>,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedFile {
    pub path: String,
    pub language: String,
    pub symbols: Vec<CodeSymbol>,
    pub imports: Vec<ImportStatement>,
    pub references: Vec<AstReference>,
    pub chunks: Vec<CodeChunk>,
}
