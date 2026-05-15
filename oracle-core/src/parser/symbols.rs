use tree_sitter::{Query, QueryCursor, Node};
use crate::parser::languages::SupportedLanguage;
use crate::parser::types::CodeSymbol;

pub fn extract_symbols(
    lang_type: &SupportedLanguage,
    root_node: Node,
    source_code: &[u8],
    file_path: &str,
) -> Vec<CodeSymbol> {
    let query_str = match lang_type {
        SupportedLanguage::Rust => "
            (function_item name: (identifier) @name) @function
            (struct_item name: (type_identifier) @name) @struct
            (enum_item name: (type_identifier) @name) @enum
            (trait_item name: (type_identifier) @name) @trait
            (impl_item type: (_) @name) @impl
        ",
        SupportedLanguage::Python => "
            (function_definition name: (identifier) @name) @function
            (class_definition name: (identifier) @name) @class
        ",
        SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::Tsx => "
            (function_declaration name: (identifier) @name) @function
            (class_declaration name: (identifier) @name) @class
            (method_definition name: (property_identifier) @name) @method
            (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @function
            (export_statement) @export
        ",
    };

    let ts_lang = lang_type.get_tree_sitter_language();
    let query = Query::new(&ts_lang, query_str).expect("Failed to create symbol query");
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, root_node, source_code);

    let mut symbols = Vec::new();
    for m in matches {
        let mut name = String::from("Unknown");
        let mut symbol_type = String::from("unknown");
        let mut node = m.nodes_for_capture_index(0).next().unwrap();

        // Capture 0 is usually the whole item, Capture 1 is usually the name
        // This depends on the query structure. Let's refine.
        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            if capture_name == "name" {
                name = capture.node.utf8_text(source_code).unwrap_or("Unknown").to_string();
            } else {
                symbol_type = capture_name.to_string();
                node = capture.node;
            }
        }

        symbols.push(CodeSymbol {
            name,
            symbol_type,
            file_path: file_path.to_string(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            content: node.utf8_text(source_code).unwrap_or("").to_string(),
        });
    }

    symbols
}

pub fn extract_references(
    lang_type: &SupportedLanguage,
    root_node: Node,
    source_code: &[u8],
) -> Vec<crate::parser::types::AstReference> {
    let query_str = match lang_type {
        SupportedLanguage::Rust => "
            (call_expression function: (identifier) @call)
            (call_expression function: (field_expression field: (field_identifier) @call))
            (type_identifier) @type
            (field_identifier) @field
        ",
        SupportedLanguage::Python => "
            (call function: (identifier) @call)
            (call function: (attribute attribute: (identifier) @call))
            (type) @type
        ",
        SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::Tsx => "
            (call_expression function: (identifier) @call)
            (call_expression function: (member_expression property: (property_identifier) @call))
            (type_identifier) @type
            (new_expression constructor: (identifier) @call)
        ",
    };

    let ts_lang = lang_type.get_tree_sitter_language();
    let query = Query::new(&ts_lang, query_str).expect("Failed to create reference query");
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, root_node, source_code);

    let mut references = Vec::new();
    for m in matches {
        for capture in m.captures {
            let node = capture.node;
            let name = node.utf8_text(source_code).unwrap_or("").to_string();
            if name.is_empty() { continue; }

            references.push(crate::parser::types::AstReference {
                name,
                ref_type: query.capture_names()[capture.index as usize].to_string(),
                line: node.start_position().row + 1,
                column: node.start_position().column + 1,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
        }
    }

    references
}
