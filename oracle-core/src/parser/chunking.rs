use crate::parser::types::{CodeSymbol, CodeChunk};
use std::collections::HashMap;

pub fn generate_chunks(symbols: &[CodeSymbol]) -> Vec<CodeChunk> {
    symbols.iter().map(|symbol| {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), symbol.symbol_type.clone());
        metadata.insert("file_path".to_string(), symbol.file_path.clone());

        CodeChunk {
            id: format!("{}-{}-{}", symbol.file_path, symbol.symbol_type, symbol.name),
            symbol_name: Some(symbol.name.clone()),
            content: symbol.content.clone(),
            start_line: symbol.start_line,
            end_line: symbol.end_line,
            metadata,
        }
    }).collect()
}
