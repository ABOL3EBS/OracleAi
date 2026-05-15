use crate::parser::languages::SupportedLanguage;
use crate::parser::types::ParsedFile;
use crate::parser::symbols::{extract_symbols, extract_references};
use crate::parser::imports::extract_imports;
use crate::parser::chunking::generate_chunks;
use anyhow::{Context, Result};
use tree_sitter::Parser;
use std::path::Path;

pub struct ParserEngine {
    parser: Parser,
}

impl ParserEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            parser: Parser::new(),
        })
    }

    pub fn parse_file<P: AsRef<Path>>(&mut self, path: P, language_name: &str) -> Result<ParsedFile> {
        let path_ref = path.as_ref();
        let source_code = std::fs::read(path_ref)
            .with_context(|| format!("Failed to read file: {:?}", path_ref))?;

        let lang_enum = SupportedLanguage::from_string(language_name)
            .context("Unsupported language for parsing")?;
        
        let ts_lang = lang_enum.get_tree_sitter_language();
        self.parser.set_language(&ts_lang)
            .context("Failed to set tree-sitter language")?;

        let tree = self.parser.parse(&source_code, None)
            .context("Failed to parse source code")?;
        
        let root_node = tree.root_node();
        let file_path_str = path_ref.to_string_lossy().to_string();

        let symbols = extract_symbols(&lang_enum, root_node, &source_code, &file_path_str);
        let imports = extract_imports(&lang_enum, root_node, &source_code);
        let references = extract_references(&lang_enum, root_node, &source_code);
        let chunks = generate_chunks(&symbols);

        Ok(ParsedFile {
            path: file_path_str,
            language: language_name.to_string(),
            symbols,
            imports,
            references,
            chunks,
        })
    }
}
