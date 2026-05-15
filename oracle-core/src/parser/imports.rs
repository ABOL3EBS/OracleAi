use tree_sitter::{Query, QueryCursor, Node};
use crate::parser::languages::SupportedLanguage;
use crate::parser::types::ImportStatement;

pub fn extract_imports(
    lang_type: &SupportedLanguage,
    root_node: Node,
    source_code: &[u8],
) -> Vec<ImportStatement> {
    let query_str = match lang_type {
        SupportedLanguage::Rust => "
            (use_declaration argument: (_) @source) @import
        ",
        SupportedLanguage::Python => "
            (import_statement) @import
            (import_from_statement) @import
        ",
        SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::Tsx => "
            (import_statement) @import
        ",
    };

    let ts_lang = lang_type.get_tree_sitter_language();
    let query = Query::new(&ts_lang, query_str).expect("Failed to create import query");
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, root_node, source_code);

    let mut imports = Vec::new();
    for m in matches {
        let node = m.nodes_for_capture_index(0).next().unwrap();
        let text = node.utf8_text(source_code).unwrap_or("").to_string();
        
        imports.push(ImportStatement {
            text,
            source: "".to_string(), // Simplified for now, can be refined with more specific queries
            line_number: node.start_position().row + 1,
        });
    }

    imports
}
