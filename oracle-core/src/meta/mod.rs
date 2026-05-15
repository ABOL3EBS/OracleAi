use crate::parser::ParseReport;
use crate::analyzer::DependencyAnalyzer;
use std::collections::{HashMap, HashSet};

pub struct MetaAnalyzer<'a> {
    report: &'a ParseReport,
    analyzer: &'a DependencyAnalyzer,
}

impl<'a> MetaAnalyzer<'a> {
    pub fn new(report: &'a ParseReport, analyzer: &'a DependencyAnalyzer) -> Self {
        Self { report, analyzer }
    }

    pub fn get_summary(&self) -> String {
        let total_files = self.report.total_files;
        let mut lang_dist = HashMap::new();
        for file in &self.report.files {
            *lang_dist.entry(file.language.clone()).or_insert(0) += 1;
        }

        let entry_points: Vec<_> = self.report.files.iter()
            .filter(|f| f.path.contains("main.rs") || f.path.contains("lib.rs") || f.path.contains("index.ts") || f.path.contains("__init__.py"))
            .map(|f| f.path.clone())
            .collect();

        let mut module_sizes: Vec<_> = self.report.files.iter()
            .map(|f| (f.path.clone(), f.symbols.len()))
            .collect();
        module_sizes.sort_by(|a, b| b.1.cmp(&a.1));

        let mut out = String::from("\n[PROJECT SUMMARY]\n");
        out.push_str(&format!("  Total Files: {}\n", total_files));
        out.push_str("  Language Distribution:\n");
        for (lang, count) in lang_dist {
            out.push_str(&format!("    - {}: {}\n", lang, count));
        }
        out.push_str("  Detected Entry Points:\n");
        for ep in entry_points {
            out.push_str(&format!("    - {}\n", ep));
        }
        out.push_str("  Largest Modules (by symbol count):\n");
        for (path, count) in module_sizes.iter().take(5) {
            out.push_str(&format!("    - {}: {} symbols\n", path, count));
        }
        out
    }

    pub fn get_health(&self) -> String {
        let nodes = self.report.files.len();
        if nodes == 0 { return "No files analyzed.".to_string(); }

        let mut total_edges = 0;
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for (dep, dependents) in &self.analyzer.file_dependencies {
            total_edges += dependents.len();
            for _d in dependents {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        // Circular Dependency Detection (Heuristic DFS)
        let cycles = self.detect_cycles();
        
        // Density: actual edges / max possible edges
        let density = total_edges as f32 / (nodes * (nodes - 1)) as f32;
        
        let mut coupling: Vec<_> = in_degree.iter().collect();
        coupling.sort_by(|a, b| b.1.cmp(a.1));

        let score = (100.0 - (cycles.len() as f32 * 10.0) - (density * 50.0)).max(0.0).min(100.0);

        let mut out = String::from("\n[ARCHITECTURE HEALTH]\n");
        out.push_str(&format!("  Structure Score: {:.1}/100\n", score));
        out.push_str(&format!("  Graph Density: {:.4}\n", density));
        out.push_str(&format!("  Circular Dependencies: {}\n", cycles.len()));
        for cycle in cycles.iter().take(3) {
            out.push_str(&format!("    - Cycle: {}\n", cycle.join(" -> ")));
        }
        out.push_str("  Highest Coupling (Incoming References):\n");
        for (path, count) in coupling.iter().take(3) {
            out.push_str(&format!("    - {}: {}\n", path, count));
        }
        
        if score < 70.0 {
            out.push_str("\n[RISK AREAS]\n");
            if !cycles.is_empty() { out.push_str("  - Refactor circular dependencies to improve modularity.\n"); }
            if density > 0.3 { out.push_str("  - High graph density suggests potential over-coupling.\n"); }
        }

        out
    }

    pub fn get_meta(&self) -> String {
        let entry_points: Vec<_> = self.report.files.iter()
            .filter(|f| f.path.contains("main.rs") || f.path.contains("lib.rs"))
            .collect();

        let mut core_modules = HashSet::new();
        for ep in &entry_points {
            if let Some(deps) = self.analyzer.file_dependencies.get(&ep.path) {
                for d in deps { core_modules.insert(d.clone()); }
            }
        }

        let mut out = String::from("\n[PROJECT INTROSPECTION]\n");
        out.push_str("[ENTRY POINTS]\n");
        for ep in &entry_points {
            out.push_str(&format!("  - {}\n", ep.path));
        }

        out.push_str("\n[CORE MODULES (Direct Ep Dependencies)]\n");
        for cm in core_modules {
            out.push_str(&format!("  - {}\n", cm));
        }

        out.push_str("\n[MAIN SYSTEMS]\n");
        let mut folders = HashMap::new();
        for file in &self.report.files {
            let path = std::path::Path::new(&file.path);
            if let Some(parent) = path.parent() {
                let folder = parent.to_string_lossy().to_string();
                if folder != "." && !folder.is_empty() {
                    *folders.entry(folder).or_insert(0) += file.symbols.len();
                }
            }
        }
        let mut systems: Vec<_> = folders.iter().collect();
        systems.sort_by(|a, b| b.1.cmp(a.1));
        for (sys, _) in systems.iter().take(4) {
            out.push_str(&format!("  - {}\n", sys));
        }

        out.push_str("\n[DATA FLOW OVERVIEW]\n");
        out.push_str("  The system logic flows from defined entry points through core modules into specialized subsystems.\n");
        out.push_str("  Architecture appears to be a modular dependency graph built on deterministic AST references.\n");

        out
    }

    fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut on_stack = HashSet::new();

        for file in &self.report.files {
            if !visited.contains(&file.path) {
                self.cycle_dfs(&file.path, &mut visited, &mut stack, &mut on_stack, &mut cycles);
            }
        }
        cycles
    }

    fn cycle_dfs(&self, node: &String, visited: &mut HashSet<String>, stack: &mut Vec<String>, on_stack: &mut HashSet<String>, cycles: &mut Vec<Vec<String>>) {
        visited.insert(node.clone());
        stack.push(node.clone());
        on_stack.insert(node.clone());

        if let Some(neighbors) = self.analyzer.file_dependencies.get(node) {
            for neighbor in neighbors {
                if on_stack.contains(neighbor) {
                    let mut cycle = Vec::new();
                    let mut found = false;
                    for n in stack.iter() {
                        if n == neighbor { found = true; }
                        if found { cycle.push(n.clone()); }
                    }
                    cycle.push(neighbor.clone());
                    cycles.push(cycle);
                } else if !visited.contains(neighbor) {
                    self.cycle_dfs(neighbor, visited, stack, on_stack, cycles);
                }
            }
        }

        on_stack.remove(node);
        stack.pop();
    }
}
