---
name: repin
description: Fast codebase intelligence engine for symbol search, dependency tracing, blast-radius impact analysis, file AST inspection, and token-budgeted context packing for coding agents. Use when navigating large codebases, analyzing refactoring impact, discovering call paths, or assembling minimal relevant context.
license: Apache-2.0 OR MIT
compatibility: Requires Linux x86_64 and Repin CLI (repin).
---

# Repin Agent Skill

Repin is a repository intelligence engine designed for autonomous coding agents and developers. It indexes symbol graphs, AST definitions, call hierarchies, and full-text search into an embedded SQLite database backed by a background daemon.

Use `repin` instead of brute-force directory traversal or unbounded grep when you need exact symbols, dependency graphs, refactoring blast radius, or token-budgeted prompt context.

---

## 1. Quick Setup & Workspace Verification

Before running queries, check if the workspace is initialized:

```bash
# Check if Repin is installed and print version
repin --version

# Initialize current repository (creates .repin/ and indexes codebase)
repin init

# If already initialized, sync latest VCS worktree changes
repin sync

# Check daemon and index status
repin status
```

*Note: For projects with existing `.repin`, run `repin sync` to update the graph incrementally after file modifications.*

---

## 2. Discovery & Search Workflow

### A. Find Symbols and Definitions

When searching for functions, types, traits, classes, or interfaces:

```bash
# Search graph for symbol declarations
repin search -g "<SYMBOL_NAME>"

# Hybrid search combining graph symbols and full-text search (default)
repin search --hybrid "<QUERY>"

# Exact regular-expression search across the working tree
repin search -r "<REGEX>"
```

### B. Inspect File Structure & AST Definitions

Inspect declared symbols and outlines without reading whole files:

```bash
# Print file's structural AST outline (functions, classes, methods, line ranges)
repin inspect path/to/file.rs

# Resolve exact symbol at coordinate (1-based line & column)
repin at-position path/to/file.rs 42 10
```

---

## 3. Impact Analysis & Blast Radius (ADR-025)

Before modifying, refactoring, or deleting a symbol or file, analyze what will break:

```bash
# Analyze upstream callers and downstream dependencies (default depth: 3)
repin impact "<SYMBOL_OR_FILE>"

# Adjust traversal depth
repin impact "<SYMBOL_OR_FILE>" --max-depth 5

# Machine-readable JSON output for programmatic evaluation
repin impact "<SYMBOL_OR_FILE>" --json
```

**JSON Output Structure:**

```json
{
  "target": "repin_core::store::SqliteStore",
  "depth": 3,
  "upstream_dependents": [ ... ],
  "downstream_dependencies": [ ... ]
}
```

---

## 4. Path & Relationship Tracing

Trace how two components connect or identify direct neighbors:

```bash
# Find shortest call/dependency path between two entities
repin path "<SOURCE_SYMBOL>" "<TARGET_SYMBOL>"

# Machine-readable JSON path
repin path "<SOURCE_SYMBOL>" "<TARGET_SYMBOL>" --json

# Inspect 1-hop callers, callees, and definitions
repin neighbors "<SYMBOL_NAME>"

# Inspect detailed entity record
repin entity "<SYMBOL_NAME>"
```

---

## 5. Budgeted Context Packing for LLMs & Tasks

Assemble dense, relevant source ranges packed to fit within a strict byte/token budget:

```bash
# Assemble context for a user task or natural-language query within budget
repin context "How does daemon client connection negotiate protocol version?" --budget 32768

# Tailor context generation:
# --padding-lines <N>       Add surrounding context lines
# --no-blast-radius         Exclude transitive caller/callee definitions
# --no-verbatim-source      Include structural outlines only without code blocks
repin context "sqlite WAL checkpointing" --budget 16384 --padding-lines 3

# Assemble review context for changed files since a base revision
repin review-context --since 1 --budget 65536
```

---

## 6. Installation & Updates

```bash
# Install Repin binary and bundled usage documentation
repin install

# Check for updates on GitHub
repin update --check

# Upgrade to latest release
repin update
```

---

## 7. Recommended Agent Decision Tree

1. **Need to locate a function/class?**
   → `repin search -g "<NAME>"`
2. **Need to inspect a file's public API or functions?**
   → `repin inspect <PATH>`
3. **Planning a refactor / breaking change?**
   → `repin impact "<TARGET>" --json`
4. **Tracing how symbol A calls or reaches symbol B?**
   → `repin path "<FROM>" "<TO>"`
5. **Need relevant code context for LLM prompt within token budget?**
   → `repin context "<TASK>" --budget <BYTES>`
6. **Code modified?**
   → `repin sync` to update graph instantly.
