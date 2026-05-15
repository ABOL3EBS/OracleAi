use crate::analyzer::DependencyAnalyzer;
use crate::parser::ParseReport;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnifiedNode {
    File(String),
    Symbol { name: String, file_path: String },
}

#[derive(Debug, Clone)]
pub struct ReasoningStep {
    pub from: UnifiedNode,
    pub to: UnifiedNode,
    pub relationship: String,
    pub weight: f32,
}

pub struct CompressedContext {
    pub seed_node: UnifiedNode,
    pub primary_flow: Vec<ReasoningStep>,
    pub supporting_paths: Vec<Vec<ReasoningStep>>,
    pub key_nodes: HashSet<UnifiedNode>,
}

pub struct ReasoningEngine {
    analyzer: DependencyAnalyzer,
    symbol_to_file: HashMap<String, Vec<String>>,
}

impl ReasoningEngine {
    pub fn new(report: &ParseReport) -> Self {
        let mut analyzer = DependencyAnalyzer::new();
        analyzer.build_maps(report);

        let mut symbol_to_file = HashMap::new();
        for file in &report.files {
            for symbol in &file.symbols {
                symbol_to_file
                    .entry(symbol.name.clone())
                    .or_insert_with(Vec::new)
                    .push(file.path.clone());
            }
        }

        Self {
            analyzer,
            symbol_to_file,
        }
    }

    pub fn reason(&self, query: &str) -> Option<CompressedContext> {
        let seed = self.identify_seed(query)?;
        let mut queue = VecDeque::new();
        let mut all_paths = Vec::new();

        queue.push_back((seed.clone(), 0, Vec::<ReasoningStep>::new(), HashSet::from([seed.clone()])));

        while let Some((current_node, depth, current_path, visited)) = queue.pop_front() {
            if depth >= 4 { 
                if !current_path.is_empty() {
                    all_paths.push(current_path);
                }
                continue; 
            }

            let neighbors = self.get_neighbors(&current_node);
            if neighbors.is_empty() && !current_path.is_empty() {
                all_paths.push(current_path.clone());
            }

            for (neighbor, rel, weight) in neighbors {
                if !visited.contains(&neighbor) {
                    let mut next_path = current_path.clone();
                    let step = ReasoningStep {
                        from: current_node.clone(),
                        to: neighbor.clone(),
                        relationship: rel,
                        weight: weight * (0.7f32.powi(depth as i32)), // Depth decay
                    };
                    next_path.push(step);
                    
                    let mut next_visited = visited.clone();
                    next_visited.insert(neighbor.clone());
                    
                    queue.push_back((neighbor, depth + 1, next_path, next_visited));
                }
            }
        }

        Some(self.compress_reasoning(seed, all_paths))
    }

    fn compress_reasoning(&self, seed: UnifiedNode, mut paths: Vec<Vec<ReasoningStep>>) -> CompressedContext {
        // Score paths by summing weights
        paths.sort_by(|a, b| {
            let score_a: f32 = a.iter().map(|s| s.weight).sum();
            let score_b: f32 = b.iter().map(|s| s.weight).sum();
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take the absolute best path as primary
        let primary_flow = paths.first().cloned().unwrap_or_default();
        
        // Supporting paths should be distinct from primary
        let mut supporting_paths = Vec::new();
        let primary_targets: HashSet<_> = primary_flow.iter().map(|s| s.to.clone()).collect();
        
        for path in paths.iter().skip(1) {
            let path_targets: HashSet<_> = path.iter().map(|s| s.to.clone()).collect();
            // If it's sufficiently different, add it
            if path_targets.intersection(&primary_targets).count() < path_targets.len() / 2 {
                supporting_paths.push(path.clone());
                if supporting_paths.len() >= 3 { break; }
            }
        }

        let mut key_nodes = HashSet::new();
        key_nodes.insert(seed.clone());
        for step in &primary_flow {
            key_nodes.insert(step.to.clone());
        }

        CompressedContext {
            seed_node: seed,
            primary_flow,
            supporting_paths,
            key_nodes,
        }
    }

    fn identify_seed(&self, query: &str) -> Option<UnifiedNode> {
        for (symbol, files) in &self.symbol_to_file {
            if query.to_lowercase().contains(&symbol.to_lowercase()) {
                return Some(UnifiedNode::Symbol {
                    name: symbol.clone(),
                    file_path: files[0].clone(),
                });
            }
        }

        for file in self.analyzer.file_dependencies.keys() {
            let file_lower = file.to_lowercase();
            let query_lower = query.to_lowercase();
            if file_lower.contains(&query_lower) || query_lower.contains(&file_lower) {
                return Some(UnifiedNode::File(file.clone()));
            }
        }

        None
    }

    fn get_neighbors(&self, node: &UnifiedNode) -> Vec<(UnifiedNode, String, f32)> {
        let mut neighbors = Vec::new();

        match node {
            UnifiedNode::File(path) => {
                if let Some(deps) = self.analyzer.file_dependencies.get(path) {
                    for dep in deps {
                        neighbors.push((UnifiedNode::File(dep.clone()), "is imported by".to_string(), 0.6));
                    }
                }
                for (symbol, files) in &self.symbol_to_file {
                    if files.contains(path) {
                        neighbors.push((UnifiedNode::Symbol { name: symbol.clone(), file_path: path.clone() }, "defines".to_string(), 0.4));
                    }
                }
            }
            UnifiedNode::Symbol { name, file_path } => {
                if let Some(usages) = self.analyzer.symbol_usages.get(name) {
                    for usage in usages {
                        neighbors.push((
                            UnifiedNode::File(usage.file_path.clone()),
                            "is used by".to_string(),
                            1.0, 
                        ));
                    }
                }
                neighbors.push((UnifiedNode::File(file_path.clone()), "is defined in".to_string(), 0.9));
            }
        }

        neighbors
    }

    pub fn format_compressed(&self, context: CompressedContext) -> String {
        let mut out = String::new();
        out.push_str(&format!("\n[REASONING SEED]: {:?}\n", context.seed_node));
        out.push_str("====================================================\n");

        out.push_str("\n[PRIMARY FLOW]\n");
        if context.primary_flow.is_empty() {
            out.push_str("  No clear execution flow identified.\n");
        } else {
            for step in &context.primary_flow {
                out.push_str(&format!("  {:?} --({})--> {:?}\n", step.from, step.relationship, step.to));
            }
        }

        if !context.supporting_paths.is_empty() {
            out.push_str("\n[SUPPORTING PATHS]\n");
            for (i, path) in context.supporting_paths.iter().enumerate() {
                let chain: Vec<String> = path.iter().map(|s| format!("{:?}", s.to)).collect();
                out.push_str(&format!("  {}. Path: {}\n", i + 1, chain.join(" -> ")));
            }
        }

        out.push_str("\n[KEY NODES]\n");
        for node in &context.key_nodes {
            out.push_str(&format!("  - {:?}\n", node));
        }

        out.push_str("\n[IMPACT SUMMARY]\n");
        out.push_str(&format!(
            "  The reasoning engine identified a core architectural backbone involving {} nodes.\n",
            context.key_nodes.len()
        ));
        out.push_str("  Modification of these nodes would cause a structural ripple effect across the identified flows.\n");

        out
    }
}
