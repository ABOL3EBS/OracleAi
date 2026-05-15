use crate::analyzer::{DependencyAnalyzer, SymbolReference};
use crate::parser::ParseReport;
use crate::parser::types::CodeSymbol;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct RefactorRisk {
    pub file_path: String,
    pub risk_type: String, // "Ambiguity", "Shadowing", "Alias", "Unresolved"
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefactorAnalysisReport {
    pub target_symbol: String,
    pub target_file: String,
    pub target_type: String,
    pub safe_references_count: usize,
    pub risks: Vec<RefactorRisk>,
    pub affected_files_by_depth: HashMap<usize, Vec<String>>,
    pub confidence_score: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefactorEdit {
    pub file_path: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefactorExecutionReport {
    pub target_symbol: String,
    pub new_symbol: String,
    pub edits_applied: usize,
    pub affected_files: Vec<String>,
    pub risks_detected: Vec<RefactorRisk>,
    pub confidence_score: f32,
}

pub struct RefactorEngine {
    analyzer: DependencyAnalyzer,
    report: ParseReport,
}

impl RefactorEngine {
    pub fn new(report: ParseReport) -> Self {
        let mut analyzer = DependencyAnalyzer::new();
        analyzer.build_maps(&report);
        Self { analyzer, report }
    }

    pub fn plan_rename(&self, old_symbol: &str, new_symbol: &str) -> Result<(Vec<RefactorEdit>, RefactorAnalysisReport), String> {
        let analysis = self.analyze_rename(old_symbol, new_symbol)
            .ok_or_else(|| format!("Could not find target symbol '{}'", old_symbol))?;

        if analysis.confidence_score < 70.0 {
            return Err(format!("Aborting refactor: Confidence score too low ({:.1}%). High risks detected.", analysis.confidence_score));
        }

        let mut edits = Vec::new();

        // 1. Definition edit
        let definition = self.find_definition(old_symbol).unwrap();
        edits.push(RefactorEdit {
            file_path: definition.file_path.clone(),
            start_byte: definition.start_byte,
            end_byte: definition.end_byte, // This might be the whole block, wait.
            // Actually CodeSymbol start/end byte is the whole definition. 
            // We need to find the name within the definition or just use its name field if we had its byte range.
            // For now, let's assume we need precise definition identifier location.
            // To be safe, we use the AstReferences in the same file that match the definition name and location.
            old_text: old_symbol.to_string(),
            new_text: new_symbol.to_string(),
        });
        
        // Wait, CodeSymbol's start_byte is the WHOLE block (e.g. pub struct Scanner { ... }).
        // We only want to rename the identifier.
        // Let's refine definition edit finding:
        let def_file = self.report.files.iter().find(|f| f.path == definition.file_path).unwrap();
        // The definition identifier is often the first identifier in the block or we search for it.
        // For simplicity and safety, we look for an AstReference that covers the definition's name at its start line.
        let def_ref = def_file.references.iter().find(|r| r.name == old_symbol && r.line >= definition.start_line && r.line <= definition.end_line);
        
        let actual_edits = if let Some(r) = def_ref {
            let mut e = Vec::new();
            e.push(RefactorEdit {
                file_path: definition.file_path.clone(),
                start_byte: r.start_byte,
                end_byte: r.end_byte,
                old_text: old_symbol.to_string(),
                new_text: new_symbol.to_string(),
            });
            e
        } else {
            // Fallback to definition-wide search (risky, but we already have confidence check)
            return Err("Could not locate precise definition identifier in AST.".to_string());
        };

        let mut all_edits = actual_edits;

        // 2. Reference edits
        for file in &self.report.files {
            for r in &file.references {
                if r.name == old_symbol {
                    // Check if this reference was resolved to our target
                    // Our current analyzer already does this in build_maps for symbol_usages
                    if let Some(usages) = self.analyzer.symbol_usages.get(old_symbol) {
                        if usages.iter().any(|u| u.file_path == file.path && u.line_number == r.line) {
                            all_edits.push(RefactorEdit {
                                file_path: file.path.clone(),
                                start_byte: r.start_byte,
                                end_byte: r.end_byte,
                                old_text: old_symbol.to_string(),
                                new_text: new_symbol.to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok((all_edits, analysis))
    }

    pub fn execute_edits(&self, edits: &[RefactorEdit]) -> anyhow::Result<RefactorExecutionReport> {
        let mut affected_files = HashSet::new();

        // Group edits by file
        let mut edits_by_file: HashMap<String, Vec<RefactorEdit>> = HashMap::new();
        for edit in edits {
            edits_by_file.entry(edit.file_path.clone()).or_default().push(edit.clone());
        }

        for (file_path, mut file_edits) in edits_by_file {
            let content = std::fs::read(&file_path)?;
            
            // Sort edits by byte offset DESCENDING to avoid offset shifts
            file_edits.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));

            let mut new_content = content.clone();
            for edit in file_edits {
                new_content.splice(edit.start_byte..edit.end_byte, edit.new_text.as_bytes().iter().cloned());
            }

            std::fs::write(&file_path, &new_content)?;
            affected_files.insert(file_path);
        }

        Ok(RefactorExecutionReport {
            target_symbol: edits[0].old_text.clone(),
            new_symbol: edits[0].new_text.clone(),
            edits_applied: edits.len(),
            affected_files: affected_files.into_iter().collect(),
            risks_detected: Vec::new(), // In a real system, we'd re-verify
            confidence_score: 100.0,
        })
    }

    pub fn analyze_rename(&self, old_symbol: &str, _new_symbol: &str) -> Option<RefactorAnalysisReport> {
        // 1. Find the primary definition
        let definition = self.find_definition(old_symbol)?;
        
        // 2. Identify safe references (AST-resolved)
        let safe_refs = self.analyzer.symbol_usages.get(old_symbol).cloned().unwrap_or_default();
        let safe_references_count = safe_refs.len();

        // 3. Detect Risks
        let risks = self.detect_risks(old_symbol, &definition);

        // 4. Calculate Propagation Impact
        let affected_files_by_depth = self.analyzer.calculate_symbol_impact(old_symbol, 4);

        // 5. Calculate Confidence
        let confidence_score = self.calculate_confidence(safe_references_count, &risks);

        Some(RefactorAnalysisReport {
            target_symbol: old_symbol.to_string(),
            target_file: definition.file_path.clone(),
            target_type: definition.symbol_type.clone(),
            safe_references_count,
            risks,
            affected_files_by_depth,
            confidence_score,
        })
    }

    fn find_definition(&self, symbol_name: &str) -> Option<&CodeSymbol> {
        for file in &self.report.files {
            for symbol in &file.symbols {
                if symbol.name == symbol_name {
                    return Some(symbol);
                }
            }
        }
        None
    }

    fn detect_risks(&self, symbol_name: &str, definition: &CodeSymbol) -> Vec<RefactorRisk> {
        let mut risks = Vec::new();

        // Risk 1: Duplicate symbol names in project (Ambiguity)
        for file in &self.report.files {
            for symbol in &file.symbols {
                if symbol.name == symbol_name {
                    if file.path != definition.file_path {
                        risks.push(RefactorRisk {
                            file_path: file.path.clone(),
                            risk_type: "Ambiguity".to_string(),
                            description: format!("Another '{}' is defined in this file.", symbol_name),
                        });
                    }
                }
            }
        }

        // Risk 2: Shadowing or different symbols with same name
        for file in &self.report.files {
            let has_ast_ref = file.references.iter().any(|r| r.name == symbol_name);
            let is_defined_here = file.path == definition.file_path;
            
            // If the file contains the string but NO AST reference was resolved
            // it might be a different scope or a coincidental text match
            if !has_ast_ref && !is_defined_here {
                for chunk in &file.chunks {
                    if chunk.content.contains(symbol_name) {
                        risks.push(RefactorRisk {
                            file_path: file.path.clone(),
                            risk_type: "Shadowing/Unresolved".to_string(),
                            description: format!("Text match found but not resolved by AST. Possible shadowing."),
                        });
                        break;
                    }
                }
            }
        }

        // Risk 3: Alias imports
        for file in &self.report.files {
            for import in &file.imports {
                if import.text.contains(" as ") && import.text.contains(symbol_name) {
                    risks.push(RefactorRisk {
                        file_path: file.path.clone(),
                        risk_type: "Alias".to_string(),
                        description: "Import alias detected. Manual verification required.".to_string(),
                    });
                }
            }
        }

        risks
    }

    fn calculate_confidence(&self, safe_refs: usize, risks: &[RefactorRisk]) -> f32 {
        if safe_refs == 0 && risks.is_empty() { return 0.0; }
        
        let mut score: f32 = 1.0;
        
        // Deduct for risks
        for risk in risks {
            match risk.risk_type.as_str() {
                "Ambiguity" => score -= 0.15,
                "Shadowing/Unresolved" => score -= 0.1,
                "Alias" => score -= 0.2,
                _ => score -= 0.05,
            }
        }

        score.max(0.0) * 100.0
    }

    pub fn format_report(&self, report: RefactorAnalysisReport) -> String {
        let mut out = String::new();
        out.push_str(&format!("\n[PRIMARY TARGET]\n"));
        out.push_str(&format!("  {} {}\n", report.target_type, report.target_symbol));
        out.push_str(&format!("  Location: {}\n", report.target_file));

        out.push_str(&format!("\n[SAFE RENAME REFERENCES]\n"));
        out.push_str(&format!("  {} exact AST references found and ready for rename.\n", report.safe_references_count));

        if !report.risks.is_empty() {
            out.push_str("\n[HIGH-RISK AREAS]\n");
            for risk in &report.risks {
                out.push_str(&format!("  - {} ({}): {}\n", risk.file_path, risk.risk_type, risk.description));
            }
        }

        out.push_str("\n[AFFECTED MODULES]\n");
        let mut depths: Vec<_> = report.affected_files_by_depth.keys().collect();
        depths.sort();
        for depth in depths {
            out.push_str(&format!("  Depth {}:\n", depth));
            for file in &report.affected_files_by_depth[depth] {
                out.push_str(&format!("    - {}\n", file));
            }
        }

        out.push_str(&format!("\n[CONFIDENCE]\n"));
        out.push_str(&format!("  {:.1}% safe automated rename\n", report.confidence_score));

        out
    }
}
