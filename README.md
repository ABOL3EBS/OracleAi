# OracleAI

A high-performance codebase intelligence, reasoning, and automated refactoring tool powered by AST analysis.

OracleAI provides deterministic insights into your codebase by parsing source code into Abstract Syntax Trees (ASTs), building a unified dependency graph, and offering a live, incrementally updated intelligence system.

## 🚀 Key Features

- **Deterministic AST Parsing**: Deep analysis of Rust, Python, and JavaScript/TypeScript using `tree-sitter`.
- **Live Indexing System**: Continuously watches your filesystem and incrementally updates its internal graphs without full-repo rescans.
- **Dependency & Impact Analysis**: Instantly visualize the ripple effect of any change through the dependency graph (up to 4+ layers deep).
- **Reasoning Engine**: Maps architectural "flows" and execution paths to explain how different parts of your system interact.
- **Safe Transactional Refactoring**: Perform automated symbol renames with AST-level precision. Includes risk detection for shadowing, ambiguity, and alias imports.
- **Interactive Oracle Shell**: A dedicated REPL for querying your codebase, analyzing health metrics, and exploring modules.
- **Architecture Health Monitoring**: Detects circular dependencies and calculates graph density/coupling scores.

## 🛠 Installation

Ensuring you have Rust installed, clone the repository and build:

```bash
cargo build --release
```

## 📖 Usage

### Live Mode (Watch)
Keep OracleAI running and synchronized with your file changes:
```bash
oracle-core watch <path-to-repo>
```

### Interactive Shell
Launch the REPL for a deep dive into an analyzed project:
```bash
oracle-core shell
```

### Dependency Analysis
Find out what depends on a specific symbol or file:
```bash
oracle-core deps "MyStruct"
```

### Impact Analysis
Predict the risk level of modifying a component:
```bash
oracle-core impact "src/scanner/mod.rs"
```

### Reasoning
Ask the system to explain a flow:
```bash
oracle-core reason "how does the refactor engine execute edits?"
```

## 🏗 Architecture

OracleAI is built as a modular pipeline:
1. **Scanner**: Discovery and language classification.
2. **Parser**: Tree-sitter powered AST extraction.
3. **Analyzer**: Symbol resolution and dependency mapping.
4. **Reasoner**: Graph-based flow analysis.
5. **Refactor**: Safe, transactional byte-level code modification.

## ⚖ License

MIT License - see LICENSE for details.
