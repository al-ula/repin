#!/usr/bin/env python3
"""Validate and aggregate the three Linux F4 confirmatory replicates."""

from __future__ import annotations

import json
import statistics
import sys
from pathlib import Path
from typing import Any


REPLICATE_NAMES = ["replicate-01", "replicate-02", "replicate-03"]
MODELS = ["sync", "hybrid", "async"]
WORKLOADS = [
    "crawl",
    "read_hash",
    "parse_query",
    "resolution",
    "regex",
    "context",
    "store_preparation",
    "benchmark",
    "service",
    "remote",
]
MEASUREMENT_NAMES = {
    "crawl": "throughput_crawl",
    "read_hash": "throughput_read-hash",
    "parse_query": "throughput_parse-query",
    "resolution": "throughput_resolution",
    "regex": "throughput_regex",
    "context": "throughput_context",
    "store_preparation": "throughput_store-preparation",
    "benchmark": "throughput_benchmark",
    "service": "service_throughput",
    "remote": "remote_throughput",
}
TIMING_FIELDS = {
    "elapsed_us",
    "shutdown_us",
    "max_shutdown_us",
    "service_rps",
    "remote_rps",
    "service_rps_samples",
    "remote_rps_samples",
}
EXPECTED_CASE_IDS = {
    "F4-CANCEL-crawl",
    "F4-CANCEL-read-hash",
    "F4-CANCEL-parse-query",
    "F4-CANCEL-resolution",
    "F4-CANCEL-regex",
    "F4-CANCEL-context",
    "F4-CANCEL-store-preparation",
    "F4-deadline-wins",
    "F4-timeout-wins",
    "F4-COMMIT-before-commit",
    "F4-COMMIT-during-commit",
    "F4-COMMIT-during-reconciliation",
    "F4-THROUGHPUT-crawl",
    "F4-THROUGHPUT-read-hash",
    "F4-THROUGHPUT-parse-query",
    "F4-THROUGHPUT-resolution",
    "F4-THROUGHPUT-regex",
    "F4-THROUGHPUT-context",
    "F4-THROUGHPUT-store-preparation",
    "F4-THROUGHPUT-benchmark",
    "F4-QUEUE-BOUNDS",
    "F4-WATCH-SHUTDOWN",
    "F4-SERVICE-REMOTE",
    "F4-ISOLATED-WORKER",
}


def fail(message: str) -> None:
    raise ValueError(message)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot load {path}: {error}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def relative_range(values: list[float]) -> float:
    median = statistics.median(values)
    if median == 0:
        return 0.0 if max(values) == 0 else float("inf")
    return (max(values) - min(values)) / abs(median) * 100.0


def normalized_case(case: dict[str, Any]) -> dict[str, Any]:
    details = dict(case["details"])
    for field in TIMING_FIELDS:
        details.pop(field, None)
    return {
        "id": case["id"],
        "expected": case["expected"],
        "outcome": case["outcome"],
        "details": details,
    }


def normalized_report(report: dict[str, Any]) -> dict[str, Any]:
    return {
        "overall_outcome": report["overall_outcome"],
        "hard_blocker": report["hard_blocker"],
        "models": [
            {
                "model": model["model"],
                "cases": [
                    normalized_case(case)
                    for case in sorted(model["cases"], key=lambda entry: entry["id"])
                ],
            }
            for model in sorted(report["models"], key=lambda entry: entry["model"])
        ],
    }


def model_map(report: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {model["model"]: model for model in report["models"]}


def measurement_map(model: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {measurement["name"]: measurement for measurement in model["measurements"]}


def validate_replicate(root: Path, name: str) -> dict[str, Any]:
    directory = root / name
    manifest_path = directory / "manifest.json"
    report_path = directory / "F4-report.json"
    mirror_path = directory / "F4.json"
    host_path = directory / "host.json"
    manifest = load_json(manifest_path)
    report = load_json(report_path)
    mirror = load_json(mirror_path)
    host = load_json(host_path)

    require(report_path.read_bytes() == mirror_path.read_bytes(), f"{name}: F4.json and F4-report.json differ")
    require(manifest["experiment"] == "F4", f"{name}: wrong experiment")
    require(manifest["profile"] == "Full", f"{name}: profile is not Full")
    require(manifest["workers"] == 4, f"{name}: worker count is not 4")
    require(manifest["queue_capacity"] == 32, f"{name}: queue capacity is not 32")
    require(report["experiment"] == "F4", f"{name}: report experiment is not F4")
    require(report["status"] == "complete", f"{name}: report is not complete")
    require(report["overall_outcome"] == "inconclusive", f"{name}: unexpected outcome")
    require(report["hard_blocker"] is False, f"{name}: hard blocker present")
    require(len(report["cases"]) == 72, f"{name}: expected 72 cases")
    require(len(report["measurements"]) == 54, f"{name}: expected 54 measurements")
    require(set(model_map(report)) == set(MODELS), f"{name}: model set differs")
    require(host["process_affinity"] == [0, 1, 2, 3], f"{name}: host was not pinned to CPUs 0-3")
    require(
        host["online_cpus"] and all(cpu in host["online_cpus"] for cpu in [0, 1, 2, 3]),
        f"{name}: CPUs 0-3 not online",
    )

    for model_name, model in model_map(report).items():
        require(len(model["cases"]) == 24, f"{name}/{model_name}: expected 24 cases")
        require({case["id"] for case in model["cases"]} == EXPECTED_CASE_IDS, f"{name}/{model_name}: case IDs differ")
        require(all(case["outcome"] == "pass" for case in model["cases"]), f"{name}/{model_name}: behavior failure")
        measurements = measurement_map(model)
        cancellation = [
            measurement
            for measurement in measurements.values()
            if measurement["name"].startswith("cancellation_")
        ]
        throughput = [
            measurement
            for measurement in measurements.values()
            if measurement["name"].startswith("throughput_")
            or measurement["name"] in {"service_throughput", "remote_throughput"}
        ]
        require(len(cancellation) == 7, f"{name}/{model_name}: expected 7 cancellation measurements")
        require(len(throughput) == 10, f"{name}/{model_name}: expected 10 throughput measurements")
        require(len(measurements) == 18, f"{name}/{model_name}: expected 18 measurements")
        require(all(len(measurement["samples"]) == 30 for measurement in cancellation), f"{name}/{model_name}: cancellation sample count differs")
        require(all(len(measurement["samples"]) == 5 for measurement in throughput), f"{name}/{model_name}: throughput sample count differs")

    all_cases = [case for model in report["models"] for case in model["cases"]]
    queue_details = [case["details"] for case in all_cases if case["id"] == "F4-QUEUE-BOUNDS"]
    require(len(queue_details) == 3, f"{name}: queue case count differs")
    for details in queue_details:
        require(details["max_queue"] <= 32, f"{name}: queue exceeded capacity")
        require(details["max_active"] <= 4, f"{name}: active workers exceeded four")
        require(details["worker_count"] == 4, f"{name}: configured workers differ")
        require(len(details["overflow_roots"]) == 125, f"{name}: overflow roots do not escalate to 125 roots")

    watch_details = [case["details"] for case in all_cases if case["id"] == "F4-WATCH-SHUTDOWN"]
    require(len(watch_details) == 3, f"{name}: watch case count differs")
    for details in watch_details:
        require(details["cycles"] == 100, f"{name}: watch cycle count differs")
        require(details.get("shutdown_idempotent", True) is True, f"{name}: shutdown is not idempotent")
        require(details["max_shutdown_us"] <= 250_000, f"{name}: watch shutdown exceeded 250ms")

    isolated_details = [case["details"] for case in all_cases if case["id"] == "F4-ISOLATED-WORKER"]
    require(len(isolated_details) == 3, f"{name}: isolated-worker case count differs")
    for details in isolated_details:
        require(details["terminated"] is True, f"{name}: isolated worker did not terminate")
        require(details["parser_state_returned"] is False, f"{name}: parser state leaked")
        require(details["fact_batch_returned"] is False, f"{name}: fact batch leaked")
        require(details["elapsed_us"] <= 250_000, f"{name}: isolated termination exceeded 250ms")

    return {
        "id": name,
        "path": str(directory),
        "run_id": manifest["run_id"],
        "profile": manifest["profile"],
        "binary_size_bytes": manifest["binary_size_bytes"],
        "clean_build_time_ms": manifest["clean_build_time_ms"],
        "case_count": len(report["cases"]),
        "measurement_count": len(report["measurements"]),
        "hard_blocker": report["hard_blocker"],
        "all_behavior_pass": True,
        "behavior_status": "pass",
        "host_metadata_path": str(host_path),
        "host_metadata": host,
        "manifest": {
            "platform_scope": manifest["platform_scope"],
            "target": manifest["target"],
            "os": manifest["os"],
            "architecture": manifest["architecture"],
            "fixture_seed": manifest["fixture_seed"],
        },
        "report": report,
        "normalized": normalized_report(report),
    }


def throughput_summary(replicates: list[dict[str, Any]]) -> dict[str, Any]:
    summary: dict[str, Any] = {}
    for model_name in MODELS:
        summary[model_name] = {}
        for workload in WORKLOADS:
            measurement_name = MEASUREMENT_NAMES[workload]
            values = []
            for replicate in replicates:
                measurements = measurement_map(model_map(replicate["report"])[model_name])
                measurement = measurements[measurement_name]
                details = measurement["details"]
                values.append(
                    {
                        "p50": details["p50"],
                        "p95": details["p95"],
                        "max": details["max"],
                        "raw_samples": measurement["samples"],
                    }
                )
            p50 = [value["p50"] for value in values]
            p95 = [value["p95"] for value in values]
            maximum = [value["max"] for value in values]
            summary[model_name][workload] = {
                "p50_by_replicate": p50,
                "p95_by_replicate": p95,
                "max_by_replicate": maximum,
                "raw_samples_by_replicate": [value["raw_samples"] for value in values],
                "p95_min": min(p95),
                "p95_max": max(p95),
                "p95_median": statistics.median(p95),
                "p95_cross_run_relative_range_percent": relative_range(p95),
            }
    return summary


def hybrid_benefit(summary: dict[str, Any]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for workload in ["service", "remote"]:
        sync_p95 = summary["sync"][workload]["p95_by_replicate"]
        hybrid_p95 = summary["hybrid"][workload]["p95_by_replicate"]
        benefits = [
            (hybrid - sync) / sync * 100.0 if sync else float("inf")
            for sync, hybrid in zip(sync_p95, hybrid_p95)
        ]
        result[workload] = {
            "p95_percent_by_replicate": benefits,
            "min_percent": min(benefits),
            "max_percent": max(benefits),
            "median_percent": statistics.median(benefits),
            "cross_run_relative_range_percent": relative_range(benefits),
            "all_replicates_at_least_25_percent": all(benefit >= 25.0 for benefit in benefits),
        }
    return result


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} CONFIRMATORY_ROOT AGGREGATE.json", file=sys.stderr)
        return 2

    root = Path(sys.argv[1])
    output = Path(sys.argv[2])
    try:
        replicates = [validate_replicate(root, name) for name in REPLICATE_NAMES]
        normalized_equal = all(replicate["normalized"] == replicates[0]["normalized"] for replicate in replicates[1:])
        summary = throughput_summary(replicates)
        benefits = hybrid_benefit(summary)
        relevant_ranges = [
            summary[model][workload]["p95_cross_run_relative_range_percent"]
            for model in MODELS
            for workload in ["service", "remote"]
        ] + [
            benefits[workload]["cross_run_relative_range_percent"]
            for workload in ["service", "remote"]
        ]
        strict_gate = {
            "all_replicates_behavior_pass": all(replicate["all_behavior_pass"] and not replicate["hard_blocker"] for replicate in replicates),
            "normalized_outcomes_equal": normalized_equal,
            "all_hybrid_thresholds_pass": all(
                benefits[workload]["all_replicates_at_least_25_percent"]
                for workload in ["service", "remote"]
            ),
            "all_relevant_p95_series_within_10_percent": all(value <= 10.0 for value in relevant_ranges),
            "relevant_p95_series": {
                "maximum_cross_run_relative_range_percent": max(relevant_ranges),
                "ranges": relevant_ranges,
            },
        }
        strict_gate["passed"] = all(
            [
                strict_gate["all_replicates_behavior_pass"],
                strict_gate["normalized_outcomes_equal"],
                strict_gate["all_hybrid_thresholds_pass"],
                strict_gate["all_relevant_p95_series_within_10_percent"],
            ]
        )
        recommendation = (
            "hybrid adapter-only provisional"
            if strict_gate["passed"]
            else "inconclusive; retain sync core as conservative default and revise measurement"
        )
        result = {
            "experiment": "F4",
            "confirmatory_run_id": "foundation-f4-confirm-20260818",
            "profile": "Full",
            "platform_scope": "Linux x86_64/glibc PoC only; Tier 2 and platform expansion deferred",
            "replicates": [
                {key: value for key, value in replicate.items() if key not in {"report", "normalized"}}
                for replicate in replicates
            ],
            "normalized_outcomes_equal": normalized_equal,
            "validation": {
                "replicate_count": len(replicates),
                "case_count_per_replicate": 72,
                "measurement_count_per_replicate": 54,
                "cases_per_model": 24,
                "cancellation_samples_per_measurement": 30,
                "throughput_samples_per_measurement": 5,
                "exact_case_ids": sorted(EXPECTED_CASE_IDS),
            },
            "throughput": summary,
            "hybrid_benefit": benefits,
            "strict_gate": strict_gate,
            "final_gate_result": "passed" if strict_gate["passed"] else "failed",
            "recommendation": recommendation,
            "overall_outcome": "inconclusive",
            "limitations": [
                "Linux x86_64/glibc only; Tier 2 and non-Linux execution were not implemented.",
                "The mock store demonstrates F4 atomicity semantics, not redb candidate behavior.",
                "The overall ledger remains inconclusive until a separately approved post-PoC platform-expansion plan exists.",
            ],
        }
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return 0
    except (KeyError, TypeError, ValueError) as error:
        print(f"F4 aggregation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
