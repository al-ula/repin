#!/usr/bin/env python3
"""
Automated Benchmarking & Evaluation Suite for Code Knowledge Graphs
Compares Repin, CodeGraph, and Graphify across indexing performance,
storage footprint, query retrieval latency, and context token efficiency.
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
        self.repin_bin = os.path.join(self.repo_dir, "target", "release", "repin")
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
        print("\n[1/3] Benchmarking Cold Indexing...")

        # 1. Repin
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            print("  -> Indexing with Repin...")
            t_init = run_cmd(f"{self.repin_bin} init", cwd=self.repo_dir)
            t_index = run_cmd(f"{self.repin_bin} index", cwd=self.repo_dir)
            t_status = run_cmd(f"{self.repin_bin} status", cwd=self.repo_dir)
            size = get_dir_size(os.path.join(self.repo_dir, ".repin"))
            total_time = t_init["duration_sec"] + t_index["duration_sec"]
            results["repin"] = {
                "time_sec": total_time,
                "storage_kb": size / 1024,
                "status": t_status["stdout"]
            }
            print(f"     Repin: {total_time:.3f}s | Storage: {size/1024:.1f} KB")

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

    def run_queries(self, target_symbol: str = "Engine") -> Dict[str, Any]:
        print(f"\n[2/3] Benchmarking Query Retrieval & Token Efficiency (Target: '{target_symbol}')...")
        queries = {}

        # Symbol Lookup / Exploration
        q_explore = {}
        if self.available_engines.get("repin") and os.path.exists(self.repin_bin):
            r = run_cmd(f"{self.repin_bin} search {target_symbol}", cwd=self.repo_dir)
            r_ctx = run_cmd(f"{self.repin_bin} context {target_symbol}", cwd=self.repo_dir)
            q_explore["repin_search"] = {
                "time_ms": r["duration_sec"] * 1000,
                "tokens": estimate_tokens(r["stdout"]),
                "snippet": r["stdout"][:250]
            }
            q_explore["repin_context"] = {
                "time_ms": r_ctx["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_ctx["stdout"]),
                "snippet": r_ctx["stdout"][:250]
            }

        if self.available_engines.get("codegraph"):
            r = run_cmd(f"codegraph explore {target_symbol}", cwd=self.repo_dir)
            q_explore["codegraph_explore"] = {
                "time_ms": r["duration_sec"] * 1000,
                "tokens": estimate_tokens(r["stdout"]),
                "snippet": r["stdout"][:250]
            }

        if self.available_engines.get("graphify"):
            r = run_cmd(f"graphify explain {target_symbol}", cwd=self.repo_dir)
            q_explore["graphify_explain"] = {
                "time_ms": r["duration_sec"] * 1000,
                "tokens": estimate_tokens(r["stdout"]),
                "snippet": r["stdout"][:250]
            }
        queries["symbol_explore"] = q_explore

        # Callers & Graph Traversal
        q_graph = {}
        if self.available_engines.get("codegraph"):
            r_callers = run_cmd(f"codegraph callers {target_symbol}", cwd=self.repo_dir)
            r_impact = run_cmd(f"codegraph impact {target_symbol}", cwd=self.repo_dir)
            q_graph["codegraph_callers"] = {
                "time_ms": r_callers["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_callers["stdout"]),
                "snippet": r_callers["stdout"][:250]
            }
            q_graph["codegraph_impact"] = {
                "time_ms": r_impact["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_impact["stdout"]),
                "snippet": r_impact["stdout"][:250]
            }

        if self.available_engines.get("graphify"):
            r_gods = run_cmd("graphify god-nodes --top 5", cwd=self.repo_dir)
            q_graph["graphify_god_nodes"] = {
                "time_ms": r_gods["duration_sec"] * 1000,
                "tokens": estimate_tokens(r_gods["stdout"]),
                "snippet": r_gods["stdout"][:250]
            }

        queries["graph_traversal"] = q_graph
        return queries

    def generate_report(self, indexing_res: Dict[str, Any], query_res: Dict[str, Any]) -> str:
        md = []
        md.append("# Codebase Intelligence Engine Benchmark Report\n")
        md.append(f"**Target Workspace:** `{self.repo_dir}`\n")

        # Table 1: Indexing
        md.append("## 1. Indexing & Storage Comparison\n")
        md.append("| Engine | Indexing Time | Storage Footprint | Primary Backend |")
        md.append("| :--- | :--- | :--- | :--- |")
        if "repin" in indexing_res:
            r = indexing_res["repin"]
            md.append(f"| **Repin (Release)** | **{r['time_sec']:.3f} s** | {r['storage_kb']:.1f} KB | Native Rust + SQLite FTS5 |")
        if "codegraph" in indexing_res:
            c = indexing_res["codegraph"]
            md.append(f"| **CodeGraph** | {c['time_sec']:.3f} s | {c['storage_kb']:.1f} KB | AST Indexer + SQLite WAL |")
        if "graphify" in indexing_res:
            g = indexing_res["graphify"]
            md.append(f"| **Graphify** | {g['time_sec']:.3f} s | {g['storage_kb']:.1f} KB | AST + Semantic JSON Graph |")

        # Table 2: Queries
        md.append("\n## 2. Query Latency & Token Economy\n")
        md.append("| Operation | Engine | Latency (ms) | Est. Tokens Injected | Focus |")
        md.append("| :--- | :--- | :--- | :--- | :--- |")

        exp = query_res.get("symbol_explore", {})
        if "repin_search" in exp:
            md.append(f"| Symbol Search | Repin (`search`) | {exp['repin_search']['time_ms']:.1f} ms | ~{exp['repin_search']['tokens']} tokens | Hybrid Lexical + Graph Rank Fusion |")
        if "repin_context" in exp:
            md.append(f"| Budgeted Context | Repin (`context`) | {exp['repin_context']['time_ms']:.1f} ms | ~{exp['repin_context']['tokens']} tokens | Token-budgeted snippet packing |")
        if "codegraph_explore" in exp:
            md.append(f"| Code Exploration | CodeGraph (`explore`) | {exp['codegraph_explore']['time_ms']:.1f} ms | ~{exp['codegraph_explore']['tokens']} tokens | Line-numbered source + Callers |")
        if "graphify_explain" in exp:
            md.append(f"| Entity Explanation | Graphify (`explain`) | {exp['graphify_explain']['time_ms']:.1f} ms | ~{exp['graphify_explain']['tokens']} tokens | Community node summary |")

        gt = query_res.get("graph_traversal", {})
        if "codegraph_callers" in gt:
            md.append(f"| Caller Chain | CodeGraph (`callers`) | {gt['codegraph_callers']['time_ms']:.1f} ms | ~{gt['codegraph_callers']['tokens']} tokens | Direct & indirect call sites |")
        if "codegraph_impact" in gt:
            md.append(f"| Blast Radius | CodeGraph (`impact`) | {gt['codegraph_impact']['time_ms']:.1f} ms | ~{gt['codegraph_impact']['tokens']} tokens | Affected symbols downstream |")
        if "graphify_god_nodes" in gt:
            md.append(f"| Hub Analysis | Graphify (`god-nodes`) | {gt['graphify_god_nodes']['time_ms']:.1f} ms | ~{gt['graphify_god_nodes']['tokens']} tokens | Top connected hub entities |")

        return "\n".join(md)


def main():
    parser = argparse.ArgumentParser(description="Automated Codebase Intelligence Benchmark Suite")
    parser.add_argument("--repo-dir", default=".", help="Target repository directory (default: current directory)")
    parser.add_argument("--no-build", action="store_true", help="Skip release build check")
    parser.add_argument("--no-clean", action="store_true", help="Do not clean index directories after run")
    parser.add_argument("--json-out", help="Path to write JSON benchmark results")
    parser.add_argument("--md-out", help="Path to write Markdown benchmark report")
    parser.add_argument("--symbol", default="Engine", help="Target symbol to explore (default: Engine)")
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
    query_results = suite.run_queries(target_symbol=args.symbol)

    report_md = suite.generate_report(idx_results, query_results)
    print("\n" + report_md + "\n")

    full_data = {
        "repository": suite.repo_dir,
        "indexing": idx_results,
        "queries": query_results
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
        print("\n[3/3] Cleaning up temporary index folders...")
        suite.cleanup()
        print("Done. Workspace is clean.")


if __name__ == "__main__":
    main()
