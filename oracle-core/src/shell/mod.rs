use crate::parser::{ParseReport, ParserEngine};
use crate::query::QueryLayer;
use crate::reasoner::ReasoningEngine;
use crate::analyzer::DependencyAnalyzer;
use crate::meta::MetaAnalyzer;
use crate::scanner::{RepoWatcher, FileEvent};
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::Path;

pub struct OracleSession {
    pub report: ParseReport,
    pub query_layer: QueryLayer,
    pub reasoning_engine: ReasoningEngine,
    pub analyzer: DependencyAnalyzer,
    pub parser: ParserEngine,
    pub watcher: Option<RepoWatcher>,
}

impl OracleSession {
    pub fn new(report: ParseReport) -> Result<Self> {
        let query_layer = QueryLayer::new()?;
        let reasoning_engine = ReasoningEngine::new(&report);
        let mut analyzer = DependencyAnalyzer::new();
        analyzer.build_maps(&report);
        let parser = ParserEngine::new()?;

        Ok(Self {
            report,
            query_layer,
            reasoning_engine,
            analyzer,
            parser,
            watcher: None,
        })
    }

    pub fn enable_watcher(&mut self, path: &Path) -> Result<()> {
        self.watcher = Some(RepoWatcher::new(path)?);
        Ok(())
    }

    pub fn process_events(&mut self) -> Result<()> {
        let mut events = Vec::new();
        if let Some(watcher) = &self.watcher {
            while let Ok(event) = watcher.receiver.try_recv() {
                events.push(event);
            }
        }

        if events.is_empty() {
            return Ok(());
        }

        for event in events {
            match event {
                FileEvent::Modified(path) | FileEvent::Created(path) => {
                    self.handle_file_change(&path)?;
                }
                FileEvent::Deleted(path) => {
                    let path_str = path.to_string_lossy().to_string();
                    self.analyzer.remove_file(&path_str);
                    self.report.files.retain(|f| f.path != path_str);
                    println!("\n[REMOVED] {}", path_str);
                }
            }
        }
        
        // Rebuild reasoning engine as it depends on the analyzer maps
        self.reasoning_engine = ReasoningEngine::new(&self.report);
        Ok(())
    }

    fn handle_file_change(&mut self, path: &Path) -> Result<()> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let language = crate::scanner::language::detect_language(extension);
        
        if crate::parser::languages::SupportedLanguage::from_string(&language).is_some() {
            match self.parser.parse_file(path, &language) {
                Ok(parsed_file) => {
                    let path_str = parsed_file.path.clone();
                    
                    // Update Analyzer
                    self.analyzer.update_file(&parsed_file);
                    
                    // Update Report
                    if let Some(pos) = self.report.files.iter().position(|f| f.path == path_str) {
                        self.report.files[pos] = parsed_file;
                    } else {
                        self.report.files.push(parsed_file);
                    }
                    
                    println!("\n[UPDATED] {}", path_str);
                    println!("  [REBUILT SYMBOLS] {}", self.report.files.last().unwrap().symbols.len());
                    println!("  [GRAPH PATCHED]");
                }
                Err(e) => eprintln!("Failed to parse {}: {}", path.display(), e),
            }
        }
        Ok(())
    }

    pub fn run_repl(&mut self) -> Result<()> {
        let red = "\x1b[31m";
        let reset = "\x1b[0m";
        let bold = "\x1b[1m";

        println!("Interactive Oracle Shell (type 'exit' to quit)");
        if self.watcher.is_some() {
            println!("Live mode active. System will update automatically on file changes.");
        }
        
        loop {
            self.process_events()?; // Check for updates before each prompt

            print!("{}{}oracle >{} ", red, bold, reset);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input == "exit" {
                break;
            }

            if input.is_empty() {
                continue;
            }

            let parts: Vec<&str> = input.splitn(2, ' ').collect();
            let command = parts[0];
            let args = if parts.len() > 1 { parts[1] } else { "" };

            match command {
                "ask" => self.handle_ask(args)?,
                "deps" => self.handle_deps(args)?,
                "impact" => self.handle_impact(args)?,
                "reason" => self.handle_reason(args)?,
                "summary" => self.handle_summary()?,
                "health" => self.handle_health()?,
                "meta" => self.handle_meta()?,
                _ => {
                    println!("Unknown command: {}", command);
                    println!("Available commands: ask, deps, impact, reason, summary, health, meta, exit");
                }
            }
        }

        Ok(())
    }

    fn handle_summary(&self) -> Result<()> {
        let meta = MetaAnalyzer::new(&self.report, &self.analyzer);
        println!("{}", meta.get_summary());
        Ok(())
    }

    fn handle_health(&self) -> Result<()> {
        let meta = MetaAnalyzer::new(&self.report, &self.analyzer);
        println!("{}", meta.get_health());
        Ok(())
    }

    fn handle_meta(&self) -> Result<()> {
        let meta = MetaAnalyzer::new(&self.report, &self.analyzer);
        println!("{}", meta.get_meta());
        Ok(())
    }

    fn handle_ask(&self, query: &str) -> Result<()> {
        if query.is_empty() {
            println!("Usage: ask <your question>");
            return Ok(());
        }
        let chunks: Vec<_> = self.report.files.iter()
            .flat_map(|f| f.chunks.clone())
            .collect();
        let response = self.query_layer.ask_repo(chunks, query)?;
        println!("\n{}", response);
        Ok(())
    }

    fn handle_deps(&self, target: &str) -> Result<()> {
        if target.is_empty() {
            println!("Usage: deps <SymbolName> OR <file_path.rs>");
            return Ok(());
        }

        println!("\nDependency Analysis for: \"{}\"", target);
        println!("====================================================");

        if let Some(dependents) = self.analyzer.file_dependencies.get(target) {
            println!("\nFiles that depend on this file:");
            for dep in dependents {
                println!("- {}", dep);
            }
        } else {
            let partial_matches: Vec<_> = self.analyzer.file_dependencies.keys()
                .filter(|k| k.contains(target))
                .collect();
            
            if !partial_matches.is_empty() {
                for match_key in partial_matches {
                    println!("\nDependents for file: {}", match_key);
                    for dep in &self.analyzer.file_dependencies[match_key] {
                        println!("- {}", dep);
                    }
                }
            }
        }

        if let Some(usages) = self.analyzer.symbol_usages.get(target) {
            println!("\nReferences for symbol '{}':", target);
            for usage in usages {
                println!("- {}:{} -> {}", usage.file_path, usage.line_number, usage.context.trim());
            }
        } else {
            println!("\nNo direct symbol references found for '{}'.", target);
        }

        Ok(())
    }

    fn handle_impact(&self, target: &str) -> Result<()> {
        if target.is_empty() {
            println!("Usage: impact <SymbolName> OR <file_path.rs>");
            return Ok(());
        }

        println!("\nTransitive Impact Analysis for: \"{}\"", target);
        println!("====================================================");

        let mut impact = std::collections::HashMap::new();

        if self.analyzer.file_dependencies.contains_key(target) {
            impact = self.analyzer.calculate_file_impact(target, 4);
        } else {
            let partial_file = self.analyzer.file_dependencies.keys().find(|k| k.contains(target));
            if let Some(f) = partial_file {
                println!("Assuming file: {}", f);
                impact = self.analyzer.calculate_file_impact(f, 4);
            } else if self.analyzer.symbol_usages.contains_key(target) {
                impact = self.analyzer.calculate_symbol_impact(target, 4);
            }
        }

        if impact.is_empty() {
            println!("No ripple effect detected for '{}'.", target);
            return Ok(());
        }

        let mut total_impacted = 0;
        for depth in 1..=4 {
            if let Some(files) = impact.get(&depth) {
                total_impacted += files.len();
                let label = match depth {
                    1 => "[DIRECT DEPENDENCIES]",
                    2 => "[INDIRECT DEPENDENCIES - depth 2]",
                    _ => "[INDIRECT DEPENDENCIES - depth 3+]",
                };
                println!("\n{}", label);
                for f in files {
                    println!("- {}", f);
                }
            }
        }

        println!("\n[RISK SUMMARY]");
        println!("----------------");
        println!("Total files potentially affected: {}", total_impacted);
        let risk = if total_impacted > 5 { "HIGH" } else if total_impacted > 2 { "MEDIUM" } else { "LOW" };
        println!("Change Risk Level: {}", risk);
        println!("Reasoning: The change ripples through {} layers of the dependency graph.", impact.len());

        Ok(())
    }

    fn handle_reason(&self, query: &str) -> Result<()> {
        if query.is_empty() {
            println!("Usage: reason <how does X work?>");
            return Ok(());
        }

        if let Some(context) = self.reasoning_engine.reason(query) {
            println!("{}", self.reasoning_engine.format_compressed(context));
        } else {
            println!("Could not find a starting point in the graph for query: \"{}\"", query);
        }

        Ok(())
    }
}
