mod scanner;
mod utils;
mod parser;
mod indexer;
mod query;
mod analyzer;
mod reasoner;
mod refactor;
mod transaction;
mod shell;
mod meta;

use crate::scanner::RepoScanner;
use crate::utils::format_bytes;
use crate::utils::banner::{print_banner, BannerState};
use crate::parser::{ParserEngine, ParseReport};
use crate::indexer::SemanticIndex;
use crate::query::QueryLayer;
use crate::analyzer::DependencyAnalyzer;
use crate::reasoner::ReasoningEngine;
use crate::refactor::RefactorEngine;
use crate::transaction::{RefactorTransaction, TransactionStatus};
use crate::shell::OracleSession;
use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    // Ensure transactions directory exists
    if !Path::new("transactions").exists() {
        fs::create_dir_all("transactions")?;
    }

    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 && args[1] == "query" {
        return handle_query(&args);
    }

    if args.len() > 1 && args[1] == "ask" {
        print_banner(BannerState::Querying);
        return handle_ask(&args);
    }

    if args.len() > 1 && args[1] == "deps" {
        return handle_deps(&args);
    }

    if args.len() > 1 && args[1] == "impact" {
        return handle_impact(&args);
    }

    if args.len() > 1 && args[1] == "reason" {
        return handle_reason(&args);
    }

    if args.len() > 1 && args[1] == "refactor-preview" {
        return handle_refactor_preview(&args);
    }

    if args.len() > 1 && args[1] == "refactor-apply" {
        return handle_refactor_apply(&args);
    }

    if args.len() > 1 && args[1] == "shell" {
        return handle_shell();
    }

    if args.len() > 1 && args[1] == "watch" {
        return handle_watch(&args);
    }

    let repo_path = args.get(1).map(PathBuf::from).unwrap_or_else(|| {
        env::current_dir().expect("Failed to get current directory")
    });

    if !repo_path.exists() {
        anyhow::bail!("Path does not exist: {:?}", repo_path);
    }

    print_banner(BannerState::Scanning);
    println!("Target: {:?}", repo_path);

    let scanner = RepoScanner::new();
    let scan_result = scanner.scan(&repo_path)
        .context("Failed to scan repository")?;

    println!("\nScan Summary:");
    println!("----------------");
    println!("Total Files:    {}", scan_result.total_files);
    println!("Total Size:     {}", format_bytes(scan_result.total_size));
    println!("Scan Duration:  {}ms", scan_result.scan_duration_ms);

    println!("\nDetected Languages:");
    let mut langs: Vec<_> = scan_result.languages.iter().collect();
    langs.sort_by(|a, b| b.1.cmp(a.1));
    for (lang, count) in langs {
        println!("- {}: {}", lang, count);
    }

    let json_output = serde_json::to_string_pretty(&scan_result)?;
    std::fs::write("scan_report.json", &json_output)?;
    println!("\nScan report saved to scan_report.json");

    println!("\nStarting AST Parsing...");
    let mut parser_engine = ParserEngine::new()?;
    let mut parsed_files = Vec::new();

    for file in &scan_result.files {
        if crate::parser::languages::SupportedLanguage::from_string(&file.language).is_some() {
            let full_path = repo_path.join(&file.path);
            match parser_engine.parse_file(&full_path, &file.language) {
                Ok(parsed_file) => {
                    parsed_files.push(parsed_file);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse {:?}: {}", full_path, e);
                }
            }
        }
    }

    let total_symbols: usize = parsed_files.iter().map(|f| f.symbols.len()).sum();
    let total_imports: usize = parsed_files.iter().map(|f| f.imports.len()).sum();
    let total_chunks: usize = parsed_files.iter().map(|f| f.chunks.len()).sum();

    println!("\nParse Summary:");
    println!("----------------");
    println!("Parsed Files:   {}", parsed_files.len());
    println!("Total Symbols:  {}", total_symbols);
    println!("Total Imports:  {}", total_imports);
    println!("Total Chunks:   {}", total_chunks);

    let report = ParseReport {
        total_files: parsed_files.len(),
        total_symbols,
        total_imports,
        total_chunks,
        files: parsed_files,
    };

    let parse_json = serde_json::to_string_pretty(&report)?;
    std::fs::write("parse_report.json", &parse_json)?;
    println!("\nParse report saved to parse_report.json");

    Ok(())
}

fn handle_query(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        anyhow::bail!("Usage: oracle-core query \"your question here\"");
    }
    let query = &args[2];

    println!("Loading parse_report.json...");
    let report_content = std::fs::read_to_string("parse_report.json")
        .context("Failed to read parse_report.json. Please run a scan first.")?;
    let report: ParseReport = serde_json::from_str(&report_content)?;

    let all_chunks: Vec<_> = report.files.into_iter()
        .flat_map(|f| f.chunks)
        .collect();

    if all_chunks.is_empty() {
        println!("No code chunks found to search.");
        return Ok(());
    }

    let indexer = SemanticIndex::new()?;
    let results = indexer.search(all_chunks, query, 5)?;

    println!("\nTop Semantic Matches:");
    println!("-----------------------");
    for (i, (chunk, score)) in results.iter().enumerate() {
        println!("{}. [Score: {:.4}]", i + 1, score);
        println!("   File: {}", chunk.metadata.get("file_path").unwrap_or(&"unknown".to_string()));
        println!("   Symbol: {}", chunk.symbol_name.as_deref().unwrap_or("unknown"));
        println!("   Lines: {}-{}", chunk.start_line, chunk.end_line);
        println!("   Snippet:\n{}\n", truncate_snippet(&chunk.content, 3));
        println!("-----------------------");
    }

    Ok(())
}

fn handle_ask(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        anyhow::bail!("Usage: oracle-core ask \"your question\"");
    }
    let question = &args[2];

    println!("Loading parse_report.json...");
    let report_content = std::fs::read_to_string("parse_report.json")
        .context("Failed to read parse_report.json. Please run a scan first.")?;
    let report: ParseReport = serde_json::from_str(&report_content)?;

    let all_chunks: Vec<_> = report.files.into_iter()
        .flat_map(|f| f.chunks)
        .collect();

    if all_chunks.is_empty() {
        println!("No code chunks found to search.");
        return Ok(());
    }

    let query_layer = QueryLayer::new()?;
    let response = query_layer.ask_repo(all_chunks, question)?;

    println!("\n{}", response);

    Ok(())
}

fn handle_deps(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        anyhow::bail!("Usage: oracle-core deps \"SymbolName\" OR \"file_path.rs\"");
    }
    let target = &args[2];

    println!("Loading parse_report.json...");
    let report_content = std::fs::read_to_string("parse_report.json")
        .context("Failed to read parse_report.json. Please run a scan first.")?;
    let report: ParseReport = serde_json::from_str(&report_content)?;

    let mut analyzer = DependencyAnalyzer::new();
    analyzer.build_maps(&report);

    println!("\nDependency Analysis for: \"{}\"", target);
    println!("====================================================");

    if let Some(dependents) = analyzer.file_dependencies.get(target) {
        println!("\nFiles that depend on this file:");
        for dep in dependents {
            println!("- {}", dep);
        }
    } else {
        let partial_matches: Vec<_> = analyzer.file_dependencies.keys()
            .filter(|k| k.contains(target))
            .collect();
        
        if !partial_matches.is_empty() {
            for match_key in partial_matches {
                println!("\nDependents for file: {}", match_key);
                for dep in &analyzer.file_dependencies[match_key] {
                    println!("- {}", dep);
                }
            }
        }
    }

    if let Some(usages) = analyzer.symbol_usages.get(target) {
        println!("\nReferences for symbol '{}':", target);
        for usage in usages {
            println!("- {}:{} -> {}", usage.file_path, usage.line_number, usage.context.trim());
        }
    } else {
        println!("\nNo direct symbol references found for '{}'.", target);
    }

    Ok(())
}

fn handle_impact(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        anyhow::bail!("Usage: oracle-core impact \"SymbolName\" OR \"file_path.rs\"");
    }
    let target = &args[2];

    println!("Loading parse_report.json...");
    let report_content = std::fs::read_to_string("parse_report.json")
        .context("Failed to read parse_report.json. Please run a scan first.")?;
    let report: ParseReport = serde_json::from_str(&report_content)?;

    let mut analyzer = DependencyAnalyzer::new();
    analyzer.build_maps(&report);

    println!("\nTransitive Impact Analysis for: \"{}\"", target);
    println!("====================================================");

    let mut impact = HashMap::new();

    if analyzer.file_dependencies.contains_key(target) {
        impact = analyzer.calculate_file_impact(target, 4);
    } else {
        let partial_file = analyzer.file_dependencies.keys().find(|k| k.contains(target));
        if let Some(f) = partial_file {
            println!("Assuming file: {}", f);
            impact = analyzer.calculate_file_impact(f, 4);
        } else if analyzer.symbol_usages.contains_key(target) {
            impact = analyzer.calculate_symbol_impact(target, 4);
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

fn handle_reason(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        anyhow::bail!("Usage: oracle-core reason \"how does X work?\"");
    }
    let query = &args[2];

    println!("Loading parse_report.json...");
    let report_content = std::fs::read_to_string("parse_report.json")
        .context("Failed to read parse_report.json. Please run a scan first.")?;
    let report: ParseReport = serde_json::from_str(&report_content)?;

    let engine = ReasoningEngine::new(&report);
    if let Some(context) = engine.reason(query) {
        println!("{}", engine.format_compressed(context));
    } else {
        println!("Could not find a starting point in the graph for query: \"{}\"", query);
    }

    Ok(())
}

fn handle_refactor_preview(args: &[String]) -> Result<()> {
    if args.len() < 4 {
        anyhow::bail!("Usage: oracle-core refactor-preview \"OldSymbol\" \"NewSymbol\"");
    }
    let old_symbol = &args[2];
    let new_symbol = &args[3];

    println!("Loading parse_report.json...");
    let report_content = std::fs::read_to_string("parse_report.json")
        .context("Failed to read parse_report.json. Please run a scan first.")?;
    let report: ParseReport = serde_json::from_str(&report_content)?;

    let engine = RefactorEngine::new(report);
    if let Some(analysis) = engine.analyze_rename(old_symbol, new_symbol) {
        println!("{}", engine.format_report(analysis));
    } else {
        println!("Could not find target symbol '{}' for refactor analysis.", old_symbol);
    }

    Ok(())
}

fn handle_refactor_apply(args: &[String]) -> Result<()> {
    if args.len() < 4 {
        anyhow::bail!("Usage: oracle-core refactor-apply \"OldSymbol\" \"NewSymbol\" [--apply]");
    }
    let old_symbol = &args[2];
    let new_symbol = &args[3];
    let should_apply = args.contains(&"--apply".to_string());

    println!("Loading parse_report.json...");
    let report_content = std::fs::read_to_string("parse_report.json")
        .context("Failed to read parse_report.json. Please run a scan first.")?;
    let report: ParseReport = serde_json::from_str(&report_content)?;

    let engine = RefactorEngine::new(report);
    match engine.plan_rename(old_symbol, new_symbol) {
        Ok((edits, analysis)) => {
            let tx_id = format!("TX-{}", Utc::now().timestamp_millis());
            let affected_files: Vec<String> = edits.iter().map(|e| e.file_path.clone()).collect();
            let mut transaction = RefactorTransaction::new(tx_id.clone(), affected_files);
            transaction.write_journal()?;

            println!("\n[TRANSACTION]");
            println!("  ID: {}", tx_id);

            println!("\n[REFACTOR TARGET]");
            println!("  {} → {}", old_symbol, new_symbol);
            
            println!("\n[SAFE REFERENCES]");
            println!("  {} exact AST references identified.", edits.len());

            println!("\n[FILES TO MODIFY]");
            let files: std::collections::HashSet<_> = edits.iter().map(|e| &e.file_path).collect();
            for f in files {
                println!("  - {}", f);
            }

            println!("\n[CONFIDENCE]");
            println!("  {:.1}%", analysis.confidence_score);

            if !should_apply {
                println!("\n[PREVIEW MODE]");
                println!("  To apply these changes, run with --apply flag.");
                println!("  No files have been modified.");
            } else {
                println!("\n[APPLYING EDITS]");
                let exec_report = engine.execute_edits(&edits)?;
                println!("  Successfully applied {} edits across {} files.", exec_report.edits_applied, exec_report.affected_files.len());
                
                transaction.status = TransactionStatus::Applied;
                transaction.write_journal()?;

                let journal = serde_json::to_string_pretty(&exec_report)?;
                std::fs::write("refactor_report.json", journal)?;
                println!("  Refactor journal saved to refactor_report.json");
            }
        }
        Err(e) => {
            println!("\n[STATUS] REJECTED");
            println!("  Reason: {}", e);
        }
    }

    Ok(())
}

fn handle_shell() -> Result<()> {
    println!("Loading parse_report.json...");
    let report_content = std::fs::read_to_string("parse_report.json")
        .context("Failed to read parse_report.json. Please run a scan first.")?;
    let report: ParseReport = serde_json::from_str(&report_content)?;

    let repo_path = env::current_dir().expect("Failed to get current directory");
    let mut session = OracleSession::new(report, repo_path)?;
    session.run_repl()?;

    Ok(())
}

fn handle_watch(args: &[String]) -> Result<()> {
    let repo_path = args.get(2).map(PathBuf::from).unwrap_or_else(|| {
        env::current_dir().expect("Failed to get current directory")
    });

    if !repo_path.exists() {
        anyhow::bail!("Path does not exist: {:?}", repo_path);
    }

    print_banner(BannerState::Scanning);
    println!("Target (Watch Mode): {:?}", repo_path);

    let scanner = RepoScanner::new();
    let scan_result = scanner.scan(&repo_path)
        .context("Failed to scan repository")?;

    println!("\nStarting Initial AST Parsing...");
    let mut parser_engine = ParserEngine::new()?;
    let mut parsed_files = Vec::new();

    for file in &scan_result.files {
        if crate::parser::languages::SupportedLanguage::from_string(&file.language).is_some() {
            let full_path = repo_path.join(&file.path);
            match parser_engine.parse_file(&full_path, &file.language) {
                Ok(parsed_file) => {
                    parsed_files.push(parsed_file);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse {:?}: {}", full_path, e);
                }
            }
        }
    }

    let report = ParseReport {
        total_files: parsed_files.len(),
        total_symbols: parsed_files.iter().map(|f| f.symbols.len()).sum(),
        total_imports: parsed_files.iter().map(|f| f.imports.len()).sum(),
        total_chunks: parsed_files.iter().map(|f| f.chunks.len()).sum(),
        files: parsed_files,
    };

    let mut session = OracleSession::new(report, repo_path)?;
    session.enable_watcher()?;
    session.run_repl()?;

    Ok(())
}

fn truncate_snippet(content: &str, max_lines: usize) -> String {
    content.lines()
        .take(max_lines)
        .map(|l| format!("      {}", l))
        .collect::<Vec<_>>()
        .join("\n")
}
