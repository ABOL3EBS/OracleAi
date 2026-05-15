use crate::parser::types::CodeChunk;
use anyhow::Result;
use std::collections::HashSet;

/// A lightweight, deterministic indexer that uses a term-frequency vector space.
/// This acts as a placeholder for a true semantic model while keeping the 
/// architecture and retrieval pipeline solid.
pub struct SemanticIndex;

impl SemanticIndex {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Performs a similarity search over chunks using a simple vector space model.
    pub fn search(&self, chunks: Vec<CodeChunk>, query: &str, top_k: usize) -> Result<Vec<(CodeChunk, f32)>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let query_terms = self.tokenize(query);
        
        let mut results: Vec<(CodeChunk, f32)> = chunks
            .into_iter()
            .map(|chunk| {
                let chunk_terms = self.tokenize(&chunk.content);
                let score = self.calculate_similarity(&query_terms, &chunk_terms);
                (chunk, score)
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);

        Ok(results)
    }

    /// Basic tokenizer: lowercase, alpha-numeric only, split by whitespace/symbols.
    fn tokenize(&self, text: &str) -> HashSet<String> {
        let mut tokens = HashSet::new();
        // Split by symbols first
        for part in text.split(|c: char| !c.is_alphanumeric()) {
            if part.len() < 3 { continue; }
            tokens.insert(part.to_lowercase());
            
            // Handle CamelCase/PascalCase
            let mut current = String::new();
            for c in part.chars() {
                if c.is_uppercase() && !current.is_empty() {
                    if current.len() >= 3 {
                        tokens.insert(current.to_lowercase());
                    }
                    current.clear();
                }
                current.push(c);
            }
            if current.len() >= 3 {
                tokens.insert(current.to_lowercase());
            }
        }
        tokens
    }

    /// Jaccard Similarity as a deterministic proxy for "semantic" overlap.
    fn calculate_similarity(&self, query_terms: &HashSet<String>, chunk_terms: &HashSet<String>) -> f32 {
        if query_terms.is_empty() || chunk_terms.is_empty() {
            return 0.0;
        }

        let intersection = query_terms.intersection(chunk_terms).count();
        let union = query_terms.union(chunk_terms).count();

        intersection as f32 / union as f32
    }
}
