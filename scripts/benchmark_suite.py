#!/usr/bin/env python3
"""
Automated Benchmarking & Evaluation Suite for Code Knowledge Graphs
Compares Repin, CodeGraph, and Graphify across cold/incremental indexing performance,
storage footprint, multi-channel search retrieval, graph relationship traversal,
AST micro-inspection, token-budgeted context assembly, and Precision-at-N evaluation.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from typing import Any, Dict, List, Optional


def run_cmd(cmd: str, cwd: str, capture: bool = True) -> Dict[str, Any]:
    start = time.perf_counter()
    res = subprocess.run(
        cmd,
        cwd=cwd,
        shell=True,
        capture_output=capture,
        text=True
    )
    duration = time.perf_counter() - start
    return {
        "cmd": cmd,
        "duration_sec": duration,
        "stdout": res.stdout.strip() if res.stdout else "",
        "stderr": res.stderr.strip() if res.stderr else "",
        "exit_code": res.returncode
    }


def get_dir_size(path: str) -> int:
    if not os.path.exists(path):
        return 0
    total = 0
    for dirpath, _, filenames in os.walk(path):
        for f in filenames:
            fp = os.path.join(dirpath, f)
            if not os.path.islink(fp):
                try:
                    total += os.path.getsize(fp)
                except OSError:
                    pass
    return total


def estimate_tokens(text: str) -> int:
    # Standard rule of thumb: ~4 characters per token
    return max(1, len(text) // 4) if text else 0


class BenchmarkSuite:
    def __init__(self, repo_dir: str, release_build: bool = True):
        self.repo_dir = os.path.abspath(repo_dir)
        self.release_build = release_build
        target_dir = os.environ.get("CARGO_TARGET_DIR", os.path.join(self.repo_dir, "target"))
        if not os.path.isabs(target_dir):
            target_dir = os.path.join(self.repo_dir, target_dir)
        build_target = os.environ.get("CARGO_BUILD_TARGET")
        release_dir = (
            os.path.join(target_dir, build_target, "release")
            if build_target
            else os.path.join(target_dir, "release")
        )
        self.repin_bin = os.path.join(release_dir, "repin")
        self.available_engines = self._detect_engines()

    def _detect_engines(self) -> Dict[str, bool]:
        engines = {
            "repin": os.path.exists(self.repin_bin) or shutil.which("repin") is not None,
            "codegraph": shutil.which("codegraph") is not None,
            "graphify": shutil.which("graphify") is not None,
        }
        return engines

    def cleanup(self):
        """Safely cleans up indexing directories and daemon processes."""
        # Stop daemons non-interactively
        if self.available_engines.get("codegraph"):
            subprocess.run(
                "echo y | codegraph uninit . 2>/dev/null || true",
                cwd=self.repo_dir,
                shell=True,
                capture_output=True
            )
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            subprocess.run(
                f"{self.repin_bin} uninit -f 2>/dev/null || true",
                cwd=self.repo_dir,
                shell=True,
                capture_output=True
            )
            subprocess.run(
                f"{self.repin_bin} stop 2>/dev/null || true",
                cwd=self.repo_dir,
                shell=True,
                capture_output=True
            )

        # Remove index folders
        for folder in [".codegraph", "graphify-out", ".repin"]:
            target = os.path.join(self.repo_dir, folder)
            if os.path.exists(target):
                shutil.rmtree(target, ignore_errors=True)

    def prepare(self):
        """Ensures binaries are built."""
        if self.release_build and not os.path.exists(self.repin_bin):
            print("Building repin in --release mode...")
            res = run_cmd("cargo build --release --workspace", cwd=self.repo_dir)
            if res["exit_code"] != 0:
                print(f"Warning: Failed to build repin: {res['stderr']}")

    def run_cold_indexing(self) -> Dict[str, Any]:
        results = {}
        print("\n[1/5] Benchmarking Cold Indexing & Storage Footprint...")

        # 1. Repin (Code-Only / Rust to match CodeGraph & Graphify)
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            print("  -> Indexing with Repin (Code-Only / Rust)...")
            t_init = run_cmd(f"{self.repin_bin} init --no-index", cwd=self.repo_dir)
            repin_cfg = os.path.join(self.repo_dir, ".repin", "config.toml")
            with open(repin_cfg, "w") as f:
                f.write('[indexing]\nindex_docs = false\nindex_config = false\nexclude_extensions = ["md", "toml", "json", "yml", "yaml"]\nexclude_paths = ["docs/**", "book/**"]\n')
            t_index = run_cmd(f"{self.repin_bin} index", cwd=self.repo_dir)
            t_status = run_cmd(f"{self.repin_bin} status", cwd=self.repo_dir)
            size = get_dir_size(os.path.join(self.repo_dir, ".repin"))
            total_time = t_init["duration_sec"] + t_index["duration_sec"]
            results["repin_code_only"] = {
                "time_sec": total_time,
                "storage_kb": size / 1024,
                "status": t_status["stdout"]
            }
            print(f"     Repin (Code-Only): {total_time:.3f}s | Storage: {size/1024:.1f} KB")

            # Also measure Repin Full-Corpus (Code + Specs/ADRs)
            print("  -> Indexing with Repin (Full-Corpus: Code + Docs/Specs)...")
            run_cmd(f"{self.repin_bin} uninit -f", cwd=self.repo_dir)
            # Kill any running daemon so it restarts with fresh (default) config
            subprocess.run("pkill -f 'repin daemon' || true", shell=True, capture_output=True)
            import time; time.sleep(0.2)
            t_init_full = run_cmd(f"{self.repin_bin} init --no-index", cwd=self.repo_dir)
            t_index_full = run_cmd(f"{self.repin_bin} index", cwd=self.repo_dir)
            t_status_full = run_cmd(f"{self.repin_bin} status", cwd=self.repo_dir)
            size_full = get_dir_size(os.path.join(self.repo_dir, ".repin"))
            total_time_full = t_init_full["duration_sec"] + t_index_full["duration_sec"]
            results["repin_full"] = {
                "time_sec": total_time_full,
                "storage_kb": size_full / 1024,
                "status": t_status_full["stdout"]
            }
            print(f"     Repin (Full-Corpus): {total_time_full:.3f}s | Storage: {size_full/1024:.1f} KB")

        # 2. CodeGraph
        if self.available_engines.get("codegraph"):
            print("  -> Indexing with CodeGraph...")
            cg_init = run_cmd("codegraph init .", cwd=self.repo_dir)
            cg_status = run_cmd("codegraph status .", cwd=self.repo_dir)
            size = get_dir_size(os.path.join(self.repo_dir, ".codegraph"))
            results["codegraph"] = {
                "time_sec": cg_init["duration_sec"],
                "storage_kb": size / 1024,
                "status": cg_status["stdout"]
            }
            print(f"     CodeGraph: {cg_init['duration_sec']:.3f}s | Storage: {size/1024:.1f} KB")

        # 3. Graphify
        if self.available_engines.get("graphify"):
            print("  -> Indexing with Graphify (AST + Cargo)...")
            gf_init = run_cmd("graphify extract . --code-only --cargo", cwd=self.repo_dir)
            size = get_dir_size(os.path.join(self.repo_dir, "graphify-out"))
            node_count, edge_count = 0, 0
            gf_json = os.path.join(self.repo_dir, "graphify-out", "graph.json")
            if os.path.exists(gf_json):
                try:
                    with open(gf_json) as f:
                        data = json.load(f)
                        node_count = len(data.get("nodes", []))
                        edge_count = len(data.get("links", data.get("edges", [])))
                except Exception:
                    pass
            results["graphify"] = {
                "time_sec": gf_init["duration_sec"],
                "storage_kb": size / 1024,
                "nodes": node_count,
                "edges": edge_count
            }
            print(f"     Graphify: {gf_init['duration_sec']:.3f}s | Storage: {size/1024:.1f} KB | Nodes: {node_count}, Edges: {edge_count}")

        return results

    def run_incremental_update(self) -> Dict[str, Any]:
        print("\n[2/5] Benchmarking Incremental Worktree Synchronization...")
        results = {}
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            r_clean = run_cmd(f"{self.repin_bin} update", cwd=self.repo_dir)
            results["repin_update_clean"] = {
                "time_ms": r_clean["duration_sec"] * 1000,
                "output": r_clean["stdout"]
            }
            print(f"     Repin Incremental Sync: {r_clean['duration_sec']*1000:.2f} ms")
        return results

    def run_search_modalities(self, target_symbol: str = "Engine") -> Dict[str, Any]:
        print(f"\n[3/5] Benchmarking Multi-Channel Search Modalities (Target: '{target_symbol}')...")
        results = {}
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            # 1. Lexical / FTS with limit 10
            r_lex = run_cmd(f"{self.repin_bin} search {target_symbol} --limit 10", cwd=self.repo_dir)
            results["repin_search_default"] = {
                "time_ms": r_lex["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_lex["stdout"]),
                "snippet": r_lex["stdout"][:250]
            }

            # 2. Hybrid Rank Fusion with limit 10
            r_hyb = run_cmd(f"{self.repin_bin} search --hybrid {target_symbol} --limit 10", cwd=self.repo_dir)
            results["repin_search_hybrid"] = {
                "time_ms": r_hyb["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_hyb["stdout"]),
                "snippet": r_hyb["stdout"][:250]
            }

            # 3. Pure Symbol Graph Search with limit 10
            r_graph = run_cmd(f"{self.repin_bin} search --graph {target_symbol} --limit 10", cwd=self.repo_dir)
            results["repin_search_graph"] = {
                "time_ms": r_graph["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_graph["stdout"]),
                "snippet": r_graph["stdout"][:250]
            }

            # 4. Direct Regex Worktree Search
            r_regex = run_cmd(f"{self.repin_bin} search --regex {target_symbol}", cwd=self.repo_dir)
            results["repin_search_regex"] = {
                "time_ms": r_regex["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_regex["stdout"]),
                "snippet": r_regex["stdout"][:250]
            }

        # CodeGraph Symbol Search (Default limit 10)
        if self.available_engines.get("codegraph"):
            r_cg = run_cmd(f"codegraph query {target_symbol}", cwd=self.repo_dir)
            results["codegraph_query"] = {
                "time_ms": r_cg["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_cg["stdout"]),
                "snippet": r_cg["stdout"][:250]
            }

        # Graphify Subgraph Search
        if self.available_engines.get("graphify"):
            r_gf = run_cmd(f"graphify query \"{target_symbol}\"", cwd=self.repo_dir)
            results["graphify_query"] = {
                "time_ms": r_gf["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_gf["stdout"]),
                "snippet": r_gf["stdout"][:250]
            }
        return results

    def run_graph_and_ast_queries(self, target_symbol: str = "Engine", sample_file: str = "crates/repin-engine/src/lib.rs") -> Dict[str, Any]:
        print(f"\n[4/5] Benchmarking Graph Traversal, AST Inspection & Context Assembly...")
        results = {
            "graph_traversal": {},
            "ast_inspection": {},
            "context_assembly": {}
        }

        # 1. Graph Traversal
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            r_neighbors = run_cmd(f"{self.repin_bin} neighbors {target_symbol}", cwd=self.repo_dir)
            r_entity = run_cmd(f"{self.repin_bin} entity {target_symbol}", cwd=self.repo_dir)
            r_impact_1 = run_cmd(f"{self.repin_bin} impact {target_symbol} --max-depth 1", cwd=self.repo_dir)
            r_impact_3 = run_cmd(f"{self.repin_bin} impact {target_symbol} --max-depth 3", cwd=self.repo_dir)
            r_path = run_cmd(f"{self.repin_bin} path crates/repin-cli/src/main.rs DaemonClient", cwd=self.repo_dir)
            results["graph_traversal"]["repin_neighbors"] = {
                "time_ms": r_neighbors["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_neighbors["stdout"]),
                "snippet": r_neighbors["stdout"][:250]
            }
            results["graph_traversal"]["repin_entity"] = {
                "time_ms": r_entity["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_entity["stdout"]),
                "snippet": r_entity["stdout"][:250]
            }
            results["graph_traversal"]["repin_impact_1"] = {
                "time_ms": r_impact_1["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_impact_1["stdout"]),
                "snippet": r_impact_1["stdout"][:250]
            }
            results["graph_traversal"]["repin_impact_3"] = {
                "time_ms": r_impact_3["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_impact_3["stdout"]),
                "snippet": r_impact_3["stdout"][:250]
            }
            results["graph_traversal"]["repin_path"] = {
                "time_ms": r_path["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_path["stdout"]),
                "snippet": r_path["stdout"][:250]
            }

        if self.available_engines.get("codegraph"):
            r_explore = run_cmd(f"codegraph explore {target_symbol}", cwd=self.repo_dir)
            r_callers = run_cmd(f"codegraph callers {target_symbol}", cwd=self.repo_dir)
            r_impact = run_cmd(f"codegraph impact {target_symbol}", cwd=self.repo_dir)
            results["graph_traversal"]["codegraph_explore"] = {
                "time_ms": r_explore["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_explore["stdout"]),
                "snippet": r_explore["stdout"][:250]
            }
            results["graph_traversal"]["codegraph_callers"] = {
                "time_ms": r_callers["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_callers["stdout"]),
                "snippet": r_callers["stdout"][:250]
            }
            results["graph_traversal"]["codegraph_impact"] = {
                "time_ms": r_impact["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_impact["stdout"]),
                "snippet": r_impact["stdout"][:250]
            }

        if self.available_engines.get("graphify"):
            r_explain = run_cmd(f"graphify explain {target_symbol}", cwd=self.repo_dir)
            r_gods = run_cmd("graphify god-nodes --top 5", cwd=self.repo_dir)
            results["graph_traversal"]["graphify_explain"] = {
                "time_ms": r_explain["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_explain["stdout"]),
                "snippet": r_explain["stdout"][:250]
            }
            results["graph_traversal"]["graphify_god_nodes"] = {
                "time_ms": r_gods["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_gods["stdout"]),
                "snippet": r_gods["stdout"][:250]
            }

        # 2. AST Inspection & Coordinate Resolution
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            r_inspect = run_cmd(f"{self.repin_bin} inspect {sample_file}", cwd=self.repo_dir)
            r_pos = run_cmd(f"{self.repin_bin} at-position {sample_file} 35 10", cwd=self.repo_dir)
            results["ast_inspection"]["repin_inspect"] = {
                "time_ms": r_inspect["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_inspect["stdout"]),
                "snippet": r_inspect["stdout"][:250]
            }
            results["ast_inspection"]["repin_at_position"] = {
                "time_ms": r_pos["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_pos["stdout"]),
                "snippet": r_pos["stdout"][:250]
            }

        # 3. Context & Review Assembly
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            r_ctx = run_cmd(f"{self.repin_bin} context {target_symbol}", cwd=self.repo_dir)
            r_rev = run_cmd(f"{self.repin_bin} review-context --budget 4096", cwd=self.repo_dir)
            results["context_assembly"]["repin_context"] = {
                "time_ms": r_ctx["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_ctx["stdout"]),
                "snippet": r_ctx["stdout"][:250]
            }
            results["context_assembly"]["repin_review_context"] = {
                "time_ms": r_rev["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_rev["stdout"]),
                "snippet": r_rev["stdout"][:250]
            }

        return results

    def run_eval_harness(self) -> Dict[str, Any]:
        print("\n[5/5] Running Built-in Precision-at-N Retrieval Evaluation...")
        results = {}
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            r_eval = run_cmd(f"{self.repin_bin} eval", cwd=self.repo_dir)
            results["repin_eval"] = {
                "time_ms": r_eval["duration_sec"] * 1000,
                "raw_output": r_eval["stdout"]
            }
            # Extract metrics if present
            for line in r_eval["stdout"].splitlines():
                if "Precision@1:" in line:
                    results["precision_at_1"] = line.split(":", 1)[1].strip()
                elif "Precision@5:" in line:
                    results["precision_at_5"] = line.split(":", 1)[1].strip()
                elif "Mean Reciprocal Rank:" in line:
                    results["mrr"] = line.split(":", 1)[1].strip()
                elif "Total Benchmark Queries:" in line:
                    results["total_queries"] = line.split(":", 1)[1].strip()
            print(f"     Repin Eval: P@1={results.get('precision_at_1', 'N/A')}, P@5={results.get('precision_at_5', 'N/A')}, MRR={results.get('mrr', 'N/A')}")
        return results

    def generate_report(
        self,
        indexing_res: Dict[str, Any],
        update_res: Dict[str, Any],
        search_res: Dict[str, Any],
        query_res: Dict[str, Any],
        eval_res: Dict[str, Any]
    ) -> str:
        md = []
        md.append("# Codebase Intelligence Engine Benchmark Report\n")
        md.append(f"**Target Workspace:** `{self.repo_dir}`\n")

        # 1. Indexing & Storage
        md.append("## 1. Indexing & Storage Footprint\n")
        md.append("| Engine / Scope | Cold Index Time | Incremental Update | Storage Footprint | Primary Backend |")
        md.append("| :--- | :--- | :--- | :--- | :--- |")
        up_ms = f"{update_res.get('repin_update_clean', {}).get('time_ms', 0):.1f} ms" if "repin_update_clean" in update_res else "N/A"
        if "repin_code_only" in indexing_res:
            r = indexing_res["repin_code_only"]
            md.append(f"| **Repin (Code-Only / Rust)** | **{r['time_sec']:.3f} s** | **{up_ms}** | **{r['storage_kb']:.1f} KB** | Native Rust + SQLite FTS5 |")
        if "codegraph" in indexing_res:
            c = indexing_res["codegraph"]
            md.append(f"| **CodeGraph (Rust-Only)** | {c['time_sec']:.3f} s | — | {c['storage_kb']:.1f} KB | AST Indexer + SQLite WAL |")
        if "graphify" in indexing_res:
            g = indexing_res["graphify"]
            md.append(f"| **Graphify (AST + Cargo)** | {g['time_sec']:.3f} s | — | {g['storage_kb']:.1f} KB | AST + Semantic JSON Graph |")
        if "repin_full" in indexing_res:
            rf = indexing_res["repin_full"]
            md.append(f"| **Repin (Full-Corpus: Code + Specs)** | **{rf['time_sec']:.3f} s** | **{up_ms}** | **{rf['storage_kb']:.1f} KB** | Native Rust + SQLite FTS5 + Schema Interning |")

        # 2. Cross-Engine Search & Retrieval Comparison
        md.append("\n## 2. Cross-Engine Search & Retrieval Comparison\n")
        md.append("| Engine & Search Mode | Command | Latency (ms) | Est. Tokens Injected | Ranking / Search Strategy |")
        md.append("| :--- | :--- | :--- | :--- | :--- |")
        if "repin_search_hybrid" in search_res:
            s = search_res["repin_search_hybrid"]
            md.append(f"| **Repin (Hybrid Rank Fusion)** | `repin search --hybrid --limit 10` | {s['time_ms']:.1f} ms | ~{s['tokens']} tokens | Reciprocal Rank Fusion (FTS5 + Graph Degree) |")
        if "repin_search_default" in search_res:
            s = search_res["repin_search_default"]
            md.append(f"| **Repin (Lexical FTS5)** | `repin search --limit 10` | {s['time_ms']:.1f} ms | ~{s['tokens']} tokens | Direct SQLite FTS5 Porter Stemming |")
        if "repin_search_graph" in search_res:
            s = search_res["repin_search_graph"]
            md.append(f"| **Repin (Symbol Graph)** | `repin search --graph --limit 10` | {s['time_ms']:.1f} ms | ~{s['tokens']} tokens | Exact & Substring AST Symbol Index |")
        if "repin_search_regex" in search_res:
            s = search_res["repin_search_regex"]
            md.append(f"| **Repin (Direct Regex)** | `repin search --regex` | {s['time_ms']:.1f} ms | ~{s['tokens']} tokens | Indexless Parallel Worktree Ripgrep Scan |")
        if "codegraph_query" in search_res:
            s = search_res["codegraph_query"]
            md.append(f"| **CodeGraph (Symbol Query)** | `codegraph query` | {s['time_ms']:.1f} ms | ~{s['tokens']} tokens | AST Symbol Name Index Search (Limit 10) |")
        if "graphify_query" in search_res:
            s = search_res["graphify_query"]
            md.append(f"| **Graphify (Graph Query)** | `graphify query` | {s['time_ms']:.1f} ms | ~{s['tokens']} tokens | BFS Subgraph Traversal Query |")

        # 3. Graph Traversal & AST Inspection
        md.append("\n## 3. Graph Traversal & Structural Inspection\n")
        md.append("| Operation | Engine & Command | Latency (ms) | Est. Tokens Injected | Focus |")
        md.append("| :--- | :--- | :--- | :--- | :--- |")

        gt = query_res.get("graph_traversal", {})
        if "repin_neighbors" in gt:
            md.append(f"| **In/Out Relations** | Repin (`neighbors`) | {gt['repin_neighbors']['time_ms']:.1f} ms | ~{gt['repin_neighbors']['tokens']} tokens | Upstream callers & downstream dependencies |")
        if "repin_entity" in gt:
            md.append(f"| **AST Entity Lookup** | Repin (`entity`) | {gt['repin_entity']['time_ms']:.1f} ms | ~{gt['repin_entity']['tokens']} tokens | AST range, ID, qualified name & attributes |")
        if "repin_impact_1" in gt:
            md.append(f"| **Direct Impact (1-Hop)** | Repin (`impact --max-depth 1`) | {gt['repin_impact_1']['time_ms']:.1f} ms | ~{gt['repin_impact_1']['tokens']} tokens | Direct 1-hop upstream affected callers (ADR-025) |")
        if "repin_impact_3" in gt:
            md.append(f"| **Blast Radius (3-Hop)** | Repin (`impact --max-depth 3`) | {gt['repin_impact_3']['time_ms']:.1f} ms | ~{gt['repin_impact_3']['tokens']} tokens | Transitive upstream blast radius propagation (ADR-025) |")
        if "repin_path" in gt:
            md.append(f"| **Shortest Path** | Repin (`path`) | {gt['repin_path']['time_ms']:.1f} ms | ~{gt['repin_path']['tokens']} tokens | Direct & indirect dependency chain trace (ADR-025) |")
        if "codegraph_callers" in gt:
            md.append(f"| **Caller Chain** | CodeGraph (`callers`) | {gt['codegraph_callers']['time_ms']:.1f} ms | ~{gt['codegraph_callers']['tokens']} tokens | Direct call sites |")
        if "codegraph_impact" in gt:
            md.append(f"| **Direct Impact (1-Hop)** | CodeGraph (`impact`) | {gt['codegraph_impact']['time_ms']:.1f} ms | ~{gt['codegraph_impact']['tokens']} tokens | Downstream impact propagation |")
        if "codegraph_explore" in gt:
            md.append(f"| **Code Exploration** | CodeGraph (`explore`) | {gt['codegraph_explore']['time_ms']:.1f} ms | ~{gt['codegraph_explore']['tokens']} tokens | Verbatim line-numbered source + callers |")
        if "graphify_explain" in gt:
            md.append(f"| **Entity Explanation** | Graphify (`explain`) | {gt['graphify_explain']['time_ms']:.1f} ms | ~{gt['graphify_explain']['tokens']} tokens | Community node cluster summary |")
        if "graphify_god_nodes" in gt:
            md.append(f"| **Hub Analysis** | Graphify (`god-nodes`) | {gt['graphify_god_nodes']['time_ms']:.1f} ms | ~{gt['graphify_god_nodes']['tokens']} tokens | Top connected hub entities |")

        ast = query_res.get("ast_inspection", {})
        if "repin_inspect" in ast:
            md.append(f"| **File Outline AST** | Repin (`inspect`) | {ast['repin_inspect']['time_ms']:.1f} ms | ~{ast['repin_inspect']['tokens']} tokens | Tree-sitter file outline extraction |")
        if "repin_at_position" in ast:
            md.append(f"| **Coordinate Resolve** | Repin (`at-position`) | {ast['repin_at_position']['time_ms']:.1f} ms | ~{ast['repin_at_position']['tokens']} tokens | Coordinate line/col enclosing symbol |")

        # 4. Context Assembly & Review Intelligence
        md.append("\n## 4. Context Assembly & Review Intelligence\n")
        md.append("| Strategy | Engine & Command | Latency (ms) | Est. Tokens Injected | Context Packing Mechanism |")
        md.append("| :--- | :--- | :--- | :--- | :--- |")
        ctx = query_res.get("context_assembly", {})
        if "repin_context" in ctx:
            md.append(f"| **Query Context** | Repin (`context`) | {ctx['repin_context']['time_ms']:.1f} ms | ~{ctx['repin_context']['tokens']} tokens | Token-budgeted snippet packing & reranking |")
        if "repin_review_context" in ctx:
            md.append(f"| **Review Context (ADR-016)** | Repin (`review-context`) | {ctx['repin_review_context']['time_ms']:.1f} ms | ~{ctx['repin_review_context']['tokens']} tokens | Changed file diffs + blast radius snippet packing |")

        # 5. Built-in Retrieval Evaluation
        if eval_res:
            md.append("\n## 5. Built-in Precision-at-N Retrieval Evaluation (Repin `eval`)\n")
            md.append(f"- **Total Benchmark Queries:** {eval_res.get('total_queries', 'N/A')}")
            md.append(f"- **Precision@1:** {eval_res.get('precision_at_1', 'N/A')}")
            md.append(f"- **Precision@5:** {eval_res.get('precision_at_5', 'N/A')}")
            md.append(f"- **Mean Reciprocal Rank (MRR):** {eval_res.get('mrr', 'N/A')}")

        return "\n".join(md)


def main():
    parser = argparse.ArgumentParser(description="Automated Codebase Intelligence Benchmark Suite")
    parser.add_argument("--repo-dir", default=".", help="Target repository directory (default: current directory)")
    parser.add_argument("--no-build", action="store_true", help="Skip release build check")
    parser.add_argument("--no-clean", action="store_true", help="Do not clean index directories after run")
    parser.add_argument("--json-out", help="Path to write JSON benchmark results")
    parser.add_argument("--md-out", help="Path to write Markdown benchmark report")
    parser.add_argument("--symbol", default="Engine", help="Target symbol to explore (default: Engine)")
    parser.add_argument("--sample-file", default="crates/repin-engine/src/lib.rs", help="Sample file for AST inspection")
    args = parser.parse_args()

    suite = BenchmarkSuite(repo_dir=args.repo_dir, release_build=not args.no_build)

    print("==========================================================")
    print("   AUTOMATED REPO INTELLIGENCE BENCHMARK SUITE")
    print("==========================================================")
    print(f"Repository: {suite.repo_dir}")
    print(f"Detected Engines: {suite.available_engines}")

    suite.cleanup()
    suite.prepare()

    idx_results = suite.run_cold_indexing()
    update_results = suite.run_incremental_update()
    search_results = suite.run_search_modalities(target_symbol=args.symbol)
    query_results = suite.run_graph_and_ast_queries(target_symbol=args.symbol, sample_file=args.sample_file)
    eval_results = suite.run_eval_harness()

    report_md = suite.generate_report(
        idx_results,
        update_results,
        search_results,
        query_results,
        eval_results
    )
    print("\n" + report_md + "\n")

    full_data = {
        "repository": suite.repo_dir,
        "indexing": idx_results,
        "incremental_update": update_results,
        "search_modalities": search_results,
        "queries": query_results,
        "eval": eval_results
    }

    if args.json_out:
        with open(args.json_out, "w") as f:
            json.dump(full_data, f, indent=2)
        print(f"Saved JSON report to: {args.json_out}")

    if args.md_out:
        with open(args.md_out, "w") as f:
            f.write(report_md)
        print(f"Saved Markdown report to: {args.md_out}")

    if not args.no_clean:
        print("\nCleaning up temporary index folders...")
        suite.cleanup()
        print("Done. Workspace is clean.")


if __name__ == "__main__":
    main()
