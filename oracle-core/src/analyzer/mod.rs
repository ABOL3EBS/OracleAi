use crate::parser::ParseReport;
use crate::parser::types::{ParsedFile, CodeSymbol};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolReference {
    pub file_path: String,
    pub line_number: usize,
    pub context: String,
}

pub struct DependencyAnalyzer {
    // Maps a file path to a list of files that depend on it
    pub file_dependencies: HashMap<String, Vec<String>>,
    // Maps a symbol name to a list of places where it is referenced
    pub symbol_usages: HashMap<String, Vec<SymbolReference>>,
    // Maps a symbol name to files where it is defined
    pub symbol_definitions: HashMap<String, Vec<String>>,
}

impl DependencyAnalyzer {
    pub fn new() -> Self {
        Self {
            file_dependencies: HashMap::new(),
            symbol_usages: HashMap::new(),
            symbol_definitions: HashMap::new(),
        }
    }

    pub fn build_maps(&mut self, report: &ParseReport) {
        // Clear existing maps for a fresh build
        self.file_dependencies.clear();
        self.symbol_usages.clear();
        self.symbol_definitions.clear();

        // Step 1: Build a global map of where symbols are defined
        for file in &report.files {
            for symbol in &file.symbols {
                self.symbol_definitions
                    .entry(symbol.name.clone())
                    .or_insert_with(Vec::new)
                    .push(file.path.clone());
            }
        }

        // Step 2: Resolve references in each file
        for file in &report.files {
            self.add_file_data(file);
        }
    }

    fn add_file_data(&mut self, file: &ParsedFile) {
        let mut file_deps = HashSet::new();

        // Check each AST reference found in this file
        for reference in &file.references {
            if let Some(definition_files) = self.symbol_definitions.get(&reference.name) {
                for def_file in definition_files {
                    if def_file != &file.path {
                        file_deps.insert(def_file.clone());
                        
                        self.symbol_usages
                            .entry(reference.name.clone())
                            .or_insert_with(Vec::new)
                            .push(SymbolReference {
                                file_path: file.path.clone(),
                                line_number: reference.line,
                                context: format!("AST Reference: {}", reference.ref_type),
                            });
                    }
                }
            }
        }

        // Check imports for file-level dependencies
        for _import in &file.imports {
            // This is a bit slow for incremental, but kept for consistency with original logic
            // In a larger system, we'd want a more efficient way to match imports to files
            // For now, we'll just use a simplified version if possible or pass a list of known files
        }

        // Record unique file dependencies
        for dep in file_deps {
            self.file_dependencies
                .entry(dep)
                .or_insert_with(Vec::new)
                .push(file.path.clone());
        }
    }

    pub fn remove_file(&mut self, file_path: &str) {
        // 1. Remove from file_dependencies: this file is no longer a dependent of others
        for dependents in self.file_dependencies.values_mut() {
            dependents.retain(|d| d != file_path);
        }
        // 2. Remove entries where this file was the dependency target (if file deleted)
        self.file_dependencies.remove(file_path);

        // 3. Remove from symbol_usages: references from this file are gone
        for usages in self.symbol_usages.values_mut() {
            usages.retain(|u| u.file_path != file_path);
        }

        // 4. Remove from symbol_definitions: definitions in this file are gone
        let mut empty_symbols = Vec::new();
        for (symbol, files) in self.symbol_definitions.iter_mut() {
            files.retain(|f| f != file_path);
            if files.is_empty() {
                empty_symbols.push(symbol.clone());
            }
        }
        for sym in empty_symbols {
            self.symbol_definitions.remove(&sym);
            self.symbol_usages.remove(&sym); // If no definitions, usages are orphaned
        }
    }

    pub fn update_file(&mut self, file: &ParsedFile) {
        self.remove_file(&file.path);
        
        // Add new definitions
        for symbol in &file.symbols {
            self.symbol_definitions
                .entry(symbol.name.clone())
                .or_insert_with(Vec::new)
                .push(file.path.clone());
        }

        self.add_file_data(file);
    }

    pub fn calculate_file_impact(&self, start_file: &str, max_depth: usize) -> HashMap<usize, Vec<String>> {
        let mut impact_levels = HashMap::new();
        let mut visited = HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        queue.push_back((start_file.to_string(), 0));
        visited.insert(start_file.to_string());

        while let Some((current_file, depth)) = queue.pop_front() {
            if depth > 0 {
                impact_levels.entry(depth).or_insert_with(Vec::new).push(current_file.clone());
            }

            if depth < max_depth {
                if let Some(dependents) = self.file_dependencies.get(&current_file) {
                    for dep in dependents {
                        if !visited.contains(dep) {
                            visited.insert(dep.clone());
                            queue.push_back((dep.clone(), depth + 1));
                        }
                    }
                }
            }
        }

        impact_levels
    }

    pub fn calculate_symbol_impact(&self, symbol_name: &str, max_depth: usize) -> HashMap<usize, Vec<String>> {
        let mut impact_levels = HashMap::new();
        let mut visited_files = HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        if let Some(usages) = self.symbol_usages.get(symbol_name) {
            for usage in usages {
                if !visited_files.contains(&usage.file_path) {
                    visited_files.insert(usage.file_path.clone());
                    queue.push_back((usage.file_path.clone(), 1));
                }
            }
        }

        while let Some((current_file, depth)) = queue.pop_front() {
            impact_levels.entry(depth).or_insert_with(Vec::new).push(current_file.clone());

            if depth < max_depth {
                if let Some(dependents) = self.file_dependencies.get(&current_file) {
                    for dep in dependents {
                        if !visited_files.contains(dep) {
                            visited_files.insert(dep.clone());
                            queue.push_back((dep.clone(), depth + 1));
                        }
                    }
                }
            }
        }

        impact_levels
    }
}
