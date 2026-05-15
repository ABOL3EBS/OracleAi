use crate::indexer::SemanticIndex;
use crate::parser::types::CodeChunk;
use anyhow::{Context, Result};

pub struct QueryLayer {
    index: SemanticIndex,
}

impl QueryLayer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            index: SemanticIndex::new()?,
        })
    }

    pub fn ask_repo(&self, chunks: Vec<CodeChunk>, question: &str) -> Result<String> {
        let results = self.index.search(chunks, question, 3)?;

        if results.is_empty() {
            return Ok("No relevant code found for your question.".to_string());
        }

        let mut response = String::new();
        response.push_str(&format!("Answers for: \"{}\"\n", question));
        response.push_str("====================================================\n\n");

        for (i, (chunk, score)) in results.iter().enumerate() {
            let file_path = chunk.metadata.get("file_path").map(|s| s.as_str()).unwrap_or("unknown");
            let symbol_name = chunk.symbol_name.as_deref().unwrap_or("unknown");
            let symbol_type = chunk.metadata.get("type").map(|s| s.as_str()).unwrap_or("code block");

            response.push_str(&format!("{}. Found {} '{}' in {}\n", i + 1, symbol_type, symbol_name, file_path));
            response.push_str(&format!("   Confidence: {:.4}\n", score));
            
            // Generate a factual explanation based on metadata
            response.push_str(&format!(
                "   Context: This is a {} named '{}' located on lines {}-{} of {}.\n",
                symbol_type, symbol_name, chunk.start_line, chunk.end_line, file_path
            ));
            
            response.push_str("   Implementation:\n");
            response.push_str(&format!("```rust\n{}\n```\n", self.indent_snippet(&chunk.content)));
            response.push_str("----------------------------------------------------\n\n");
        }

        Ok(response)
    }

    fn indent_snippet(&self, content: &str) -> String {
        content.lines()
            .take(10) // Show a bit more for the "ask" command
            .collect::<Vec<_>>()
            .join("\n")
    }
}
