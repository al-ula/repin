#!/usr/bin/env python3
"""Capture host state for one Linux F4 diagnostic or confirmatory run."""

from __future__ import annotations

import datetime as dt
import json
import os
import platform
import subprocess
import sys
from pathlib import Path
from typing import Any


PINNED_CPUS = [0, 1, 2, 3]


def read_text(path: str) -> str | None:
    try:
        return Path(path).read_text(encoding="utf-8").strip()
    except OSError:
        return None


def command(*args: str) -> str | None:
    try:
        result = subprocess.run(args, check=True, capture_output=True, text=True)
    except (OSError, subprocess.CalledProcessError):
        return None
    return result.stdout.strip()


def parse_cpu_list(value: str | None) -> list[int]:
    if not value:
        return []
    cpus: list[int] = []
    for item in value.split(","):
        if "-" in item:
            first, last = item.split("-", 1)
            cpus.extend(range(int(first), int(last) + 1))
        else:
            cpus.append(int(item))
    return cpus


def cpu_model() -> str | None:
    contents = read_text("/proc/cpuinfo")
    if contents:
        for line in contents.splitlines():
            if line.lower().startswith("model name") and ":" in line:
                return line.split(":", 1)[1].strip()
    return platform.processor() or None


def governors() -> list[str]:
    values: set[str] = set()
    for path in Path("/sys/devices/system/cpu").glob("cpu*/cpufreq/scaling_governor"):
        value = read_text(str(path))
        if value:
            values.add(value)
    return sorted(values)


def main() -> int:
    if len(sys.argv) not in {2, 4} or (len(sys.argv) == 4 and sys.argv[2] != "--affinity"):
        print(f"usage: {sys.argv[0]} OUTPUT.json [--affinity pinned|unpinned]", file=sys.stderr)
        return 2

    affinity_mode = sys.argv[3] if len(sys.argv) == 4 else "pinned"
    if affinity_mode not in {"pinned", "unpinned"}:
        print(f"unknown affinity mode {affinity_mode}; expected pinned or unpinned", file=sys.stderr)
        return 2
    affinity = sorted(os.sched_getaffinity(0))
    if affinity_mode == "pinned" and affinity != PINNED_CPUS:
        print(
            f"expected process affinity {PINNED_CPUS}, observed {affinity}; refusing capture",
            file=sys.stderr,
        )
        return 2

    load = os.getloadavg()
    online_text = read_text("/sys/devices/system/cpu/online")
    taskset_version = command("taskset", "--version")
    data: dict[str, Any] = {
        "schema": "f4-confirmatory-host-v1",
        "affinity_mode": affinity_mode,
        "captured_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "rustc": command("rustc", "--version"),
        "cargo": command("cargo", "--version"),
        "kernel": platform.release(),
        "system": platform.system(),
        "architecture": platform.machine(),
        "cpu_model": cpu_model(),
        "logical_cpu_count": os.cpu_count(),
        "online_cpus": parse_cpu_list(online_text),
        "process_affinity": affinity,
        "load_average": {
            "one_minute": load[0],
            "five_minutes": load[1],
            "fifteen_minutes": load[2],
        },
        "cpu_governors": governors(),
        "taskset_version": taskset_version.splitlines()[0] if taskset_version else None,
    }

    output = Path(sys.argv[1])
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
