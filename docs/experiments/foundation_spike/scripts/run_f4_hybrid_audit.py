#!/usr/bin/env python3
"""Run the fixed serial F4 hybrid-benefit diagnostic matrix."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path


CONDITIONS = ("pinned", "unpinned")
CLIENT_MODES = ("native", "matched")
ORDERS = (0, 1, 2)
PINNED_CPUS = (0, 1, 2, 3)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def run(command: list[str], env: dict[str, str] | None = None) -> None:
    subprocess.run(command, check=True, env=env, stdout=subprocess.DEVNULL)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--clean-build-ms", required=True, type=float)
    args = parser.parse_args()

    binary = args.binary.resolve()
    root = args.root.resolve()
    capture = Path(__file__).with_name("capture_f4_host.py").resolve()
    if not binary.is_file():
        raise SystemExit(f"diagnostic binary does not exist: {binary}")
    if root.exists():
        raise SystemExit(f"refusing to overwrite existing audit root: {root}")
    root.mkdir(parents=True)

    base_env = os.environ.copy()
    base_env["REPINF4_CLEAN_BUILD_MS"] = str(args.clean_build_ms)
    completed = []
    for condition in CONDITIONS:
        for client_mode in CLIENT_MODES:
            for order_index in ORDERS:
                cell_name = f"{condition}-{client_mode}-order-{order_index}"
                cell = root / cell_name
                cell.mkdir()
                host_path = cell / "host.json"
                if condition == "pinned":
                    capture_command = [
                        "taskset",
                        "--cpu-list",
                        "0-3",
                        sys.executable,
                        str(capture),
                        str(host_path),
                    ]
                    run_env = base_env
                    prefix = ["taskset", "--cpu-list", "0-3"]
                else:
                    capture_command = [sys.executable, str(capture), str(host_path), "--affinity", "unpinned"]
                    run_env = base_env
                    prefix = []
                run(capture_command, env=run_env)
                host = load_json(host_path)
                observed_affinity = host["process_affinity"]
                if condition == "pinned" and observed_affinity != list(PINNED_CPUS):
                    raise SystemExit(f"{cell_name}: pinned affinity mismatch: {observed_affinity}")
                if condition == "unpinned" and observed_affinity == list(PINNED_CPUS):
                    raise SystemExit(f"{cell_name}: unpinned process is still pinned: {observed_affinity}")
                diagnostic_command = prefix + [
                    str(binary),
                    "diagnose-hybrid",
                    "--condition",
                    condition,
                    "--client-mode",
                    client_mode,
                    "--order",
                    str(order_index),
                    "--output",
                    str(cell),
                ]
                run(diagnostic_command, env=run_env)
                probe_path = cell / "probe.json"
                if not probe_path.is_file():
                    raise SystemExit(f"{cell_name}: probe.json was not produced")
                probe = load_json(probe_path)
                if probe["condition"] != condition or probe["client_mode"] != client_mode or probe["order_index"] != order_index:
                    raise SystemExit(f"{cell_name}: probe metadata does not match the requested cell")
                completed.append(cell_name)

    print(json.dumps({"root": str(root), "cells": completed}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
