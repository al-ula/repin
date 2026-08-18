#!/usr/bin/env python3
"""Validate and classify the F4 hybrid-benefit diagnostic matrix."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import statistics
from pathlib import Path
from typing import Any


MODELS = ("sync", "hybrid", "async")
WORKLOADS = ("service", "remote")
CONDITIONS = ("pinned", "unpinned")
CLIENT_MODES = ("native", "matched")
ORDERS = (0, 1, 2)
MODEL_ORDERS = {
    0: ("sync", "hybrid", "async"),
    1: ("hybrid", "async", "sync"),
    2: ("async", "sync", "hybrid"),
}
REPLICATES = ("replicate-01", "replicate-02", "replicate-03")
EXPECTED_REQUESTS = {"service": 64, "remote": 32}
EXPECTED_CLIENT_WORKERS = {
    "native": {"sync": 4, "hybrid": 8, "async": 8},
    "matched": {"sync": 4, "hybrid": 4, "async": 4},
}
EXPECTED_RUNTIME_THREADS = {"sync": 0, "hybrid": 2, "async": 4}
EXPECTED_CONFIGURED_THREADS = {"sync": 4, "hybrid": 6, "async": 8}
DIAGNOSTIC_TIMING_FIELDS = {
    "binary_size_bytes",
    "clean_build_time_ms",
    "elapsed_us",
    "rps",
    "rps_samples",
    "p50",
    "p95",
    "max",
}


def fail(message: str) -> None:
    raise ValueError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot load {path}: {error}")


def rust_percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    require(bool(ordered), "percentile input must not be empty")
    index = math.floor((len(ordered) - 1) * fraction + 0.5)
    return ordered[index]


def relative_range(values: list[float]) -> float:
    median = statistics.median(values)
    if median == 0:
        return 0.0 if max(values) == 0 else float("inf")
    return (max(values) - min(values)) / abs(median) * 100.0


def report_metrics(path: Path) -> dict[str, dict[str, dict[str, Any]]]:
    report = load_json(path)
    require(report["experiment"] == "F4", f"{path}: wrong experiment")
    result: dict[str, dict[str, dict[str, Any]]] = {}
    for model in report["models"]:
        result[model["model"]] = {}
        measurements = {measurement["name"]: measurement for measurement in model["measurements"]}
        for workload in WORKLOADS:
            name = f"{workload}_throughput"
            measurement = measurements[name]
            raw_samples = measurement["samples"]
            require(isinstance(raw_samples, list) and raw_samples, f"{path}: {name} has no raw samples")
            require(len(raw_samples) == 5, f"{path}: {name} must retain five raw samples")
            result[model["model"]][workload] = {
                "p50": measurement["details"]["p50"],
                "p95": measurement["details"]["p95"],
                "max": measurement["details"]["max"],
                "samples": raw_samples,
            }
    require(set(result) == set(MODELS), f"{path}: model set differs")
    return result


def validate_probe(cell: Path) -> dict[str, Any]:
    probe = load_json(cell / "probe.json")
    host = load_json(cell / "host.json")
    condition = probe["condition"]
    client_mode = probe["client_mode"]
    order_index = probe["order_index"]
    require(probe["schema"] == "f4-hybrid-diagnostic-v1", f"{cell}: schema mismatch")
    require(probe["experiment"] == "F4", f"{cell}: experiment mismatch")
    require(condition in CONDITIONS, f"{cell}: condition mismatch")
    require(client_mode in CLIENT_MODES, f"{cell}: client mode mismatch")
    require(order_index in ORDERS, f"{cell}: order index mismatch")
    require(probe["model_order"] == list(MODEL_ORDERS[order_index]), f"{cell}: model order mismatch")
    require(probe["process_affinity"] == host["process_affinity"], f"{cell}: probe/host affinity mismatch")
    require(host["affinity_mode"] == condition, f"{cell}: host affinity mode mismatch")
    if condition == "pinned":
        require(probe["process_affinity"] == [0, 1, 2, 3], f"{cell}: pinned affinity mismatch")
    else:
        require(probe["process_affinity"] != [0, 1, 2, 3], f"{cell}: unpinned affinity is pinned")
    require(probe["available_workers"] == 4, f"{cell}: worker count mismatch")
    require(probe["server_queue_capacity"] == 8, f"{cell}: server queue capacity mismatch")
    require(probe["warmups"] == 2, f"{cell}: warmup count mismatch")
    require(probe["samples"] == 10, f"{cell}: sample count mismatch")
    require(probe["service_requests"] == 64, f"{cell}: service request count mismatch")
    require(probe["remote_requests"] == 32, f"{cell}: remote request count mismatch")
    require([model["model"] for model in probe["models"]] == list(MODEL_ORDERS[order_index]), f"{cell}: model report order mismatch")

    summaries = []
    for model_report in probe["models"]:
        model = model_report["model"]
        require(model_report["order_index"] == probe["model_order"].index(model), f"{cell}/{model}: execution position mismatch")
        require(model_report["configured_thread_count"] == EXPECTED_CONFIGURED_THREADS[model], f"{cell}/{model}: configured thread count mismatch")
        require(model_report["runtime_thread_count"] == EXPECTED_RUNTIME_THREADS[model], f"{cell}/{model}: runtime thread count mismatch")
        require(model_report["server_worker_count"] == 4, f"{cell}/{model}: server worker count mismatch")
        require(model_report["client_concurrency"] == EXPECTED_CLIENT_WORKERS[client_mode][model], f"{cell}/{model}: client concurrency mismatch")
        require([workload["workload"] for workload in model_report["workloads"]] == list(WORKLOADS), f"{cell}/{model}: workload order mismatch")
        for workload_report in model_report["workloads"]:
            workload = workload_report["workload"]
            requests = EXPECTED_REQUESTS[workload]
            require(workload_report["requests"] == requests, f"{cell}/{model}/{workload}: request count mismatch")
            require(workload_report["warmups"] == 2, f"{cell}/{model}/{workload}: warmup count mismatch")
            require(len(workload_report["samples"]) == 10, f"{cell}/{model}/{workload}: raw sample count mismatch")
            rps_samples = [sample["rps"] for sample in workload_report["samples"]]
            require(workload_report["rps_samples"] == rps_samples, f"{cell}/{model}/{workload}: RPS samples differ")
            require(workload_report["p50"] == rust_percentile(rps_samples, 0.50), f"{cell}/{model}/{workload}: p50 mismatch")
            require(workload_report["p95"] == rust_percentile(rps_samples, 0.95), f"{cell}/{model}/{workload}: p95 mismatch")
            require(workload_report["max"] == max(rps_samples), f"{cell}/{model}/{workload}: maximum mismatch")
            for sample in workload_report["samples"]:
                require(sample["expected_requests"] == requests, f"{cell}/{model}/{workload}: expected request mismatch")
                require(sample["client_completed"] == requests, f"{cell}/{model}/{workload}: client completion mismatch")
                require(sample["server_completed"] == requests, f"{cell}/{model}/{workload}: server completion mismatch")
                require(sample["client_errors"] == 0 and sample["server_errors"] == 0, f"{cell}/{model}/{workload}: diagnostic errors present")
                require(sample["client_max_active"] <= model_report["client_concurrency"], f"{cell}/{model}/{workload}: client active bound exceeded")
                require(sample["server_max_active"] <= 4, f"{cell}/{model}/{workload}: server active bound exceeded")
                # The client counter is a submitted-but-not-started backlog. A
                # sender can be waiting on the bounded sync_channel while a
                # worker is between recv() and dequeued(), so the observed
                # handoff backlog is bounded by channel capacity plus workers.
                require(sample["client_max_queue"] <= model_report["client_concurrency"] * 3, f"{cell}/{model}/{workload}: client queue bound exceeded")
                require(sample["server_max_queue"] <= 8, f"{cell}/{model}/{workload}: server queue bound exceeded")
                require(sample["client_workers"] == model_report["client_concurrency"], f"{cell}/{model}/{workload}: client worker metadata mismatch")
                require(sample["server_workers"] == 4, f"{cell}/{model}/{workload}: server worker metadata mismatch")
                require(sample["elapsed_us"] > 0, f"{cell}/{model}/{workload}: elapsed time is not positive")
            summaries.append(
                {
                    "condition": condition,
                    "client_mode": client_mode,
                    "order_index": order_index,
                    "model": model,
                    "workload": workload,
                    "p50": workload_report["p50"],
                    "p95": workload_report["p95"],
                    "max": workload_report["max"],
                    "rps_samples": rps_samples,
                    "rps_sample_relative_range_percent": relative_range(rps_samples),
                    "elapsed_us_samples": [sample["elapsed_us"] for sample in workload_report["samples"]],
                    "client_max_active": max(sample["client_max_active"] for sample in workload_report["samples"]),
                    "server_max_active": max(sample["server_max_active"] for sample in workload_report["samples"]),
                    "client_max_queue": max(sample["client_max_queue"] for sample in workload_report["samples"]),
                    "server_max_queue": max(sample["server_max_queue"] for sample in workload_report["samples"]),
                }
            )
    return {
        "cell": cell.name,
        "path": str(cell),
        "condition": condition,
        "client_mode": client_mode,
        "order_index": order_index,
        "host_metadata": host,
        "models": summaries,
        "normalized": normalized_probe(probe),
    }


def normalized_probe(probe: dict[str, Any]) -> dict[str, Any]:
    """Keep diagnostic invariants while excluding timing/build measurements."""

    return {
        "schema": probe["schema"],
        "experiment": probe["experiment"],
        "run_id": probe["run_id"],
        "condition": probe["condition"],
        "client_mode": probe["client_mode"],
        "order_index": probe["order_index"],
        "model_order": probe["model_order"],
        "process_affinity": probe["process_affinity"],
        "available_workers": probe["available_workers"],
        "server_queue_capacity": probe["server_queue_capacity"],
        "warmups": probe["warmups"],
        "samples": probe["samples"],
        "service_requests": probe["service_requests"],
        "remote_requests": probe["remote_requests"],
        "models": [
            {
                "model": model["model"],
                "order_index": model["order_index"],
                "configured_thread_count": model["configured_thread_count"],
                "runtime_thread_count": model["runtime_thread_count"],
                "server_worker_count": model["server_worker_count"],
                "client_concurrency": model["client_concurrency"],
                "workloads": [
                    {
                        "workload": workload["workload"],
                        "requests": workload["requests"],
                        "delay_us": workload["delay_us"],
                        "warmups": workload["warmups"],
                        "samples": [
                            {
                                key: sample[key]
                                for key in (
                                    "sample",
                                    "expected_requests",
                                    "client_completed",
                                    "server_completed",
                                    "client_errors",
                                    "server_errors",
                                    "client_max_active",
                                    "server_max_active",
                                    "client_max_queue",
                                    "server_max_queue",
                                    "client_workers",
                                    "server_workers",
                                )
                            }
                            for sample in workload["samples"]
                        ],
                    }
                    for workload in model["workloads"]
                ],
            }
            for model in probe["models"]
        ],
    }


def sha256_files(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def lookup(probes: list[dict[str, Any]], condition: str, client_mode: str, order: int, model: str, workload: str) -> dict[str, Any]:
    for probe in probes:
        if (probe["condition"], probe["client_mode"], probe["order_index"]) != (condition, client_mode, order):
            continue
        for summary in probe["models"]:
            if summary["model"] == model and summary["workload"] == workload:
                return summary
    raise ValueError(f"missing probe {condition}/{client_mode}/{order}/{model}/{workload}")


def driver_report(
    values: list[float],
    threshold: float,
    direction: str = "positive",
    sample_stability: list[float] | None = None,
) -> dict[str, Any]:
    support_values = [value for value in values if value >= threshold] if direction == "positive" else [value for value in values if value <= -threshold]
    stability = sample_stability or []
    return {
        "values": values,
        "support_count": len(support_values),
        "relative_range_percent": relative_range(values),
        "sample_stability_relative_ranges_percent": stability,
        "sample_stability_within_10_percent": all(value <= 10.0 for value in stability),
        "supported": len(support_values) >= 2 and relative_range(values) <= 10.0 and all(value <= 10.0 for value in stability),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tier1", required=True, type=Path)
    parser.add_argument("--confirmatory", required=True, type=Path)
    parser.add_argument("--probes", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    tier1_metrics = report_metrics(args.tier1 / "F4-report.json")
    confirmatory_metrics = [
        report_metrics(args.confirmatory / replicate / "F4-report.json") for replicate in REPLICATES
    ]
    cell_names = [
        f"{condition}-{client_mode}-order-{order}"
        for condition in CONDITIONS
        for client_mode in CLIENT_MODES
        for order in ORDERS
    ]
    probes = [validate_probe(args.probes / cell) for cell in cell_names]

    baseline_comparison: dict[str, Any] = {}
    for model in MODELS:
        baseline_comparison[model] = {}
        for workload in WORKLOADS:
            baseline_p95 = tier1_metrics[model][workload]["p95"]
            confirmatory_p95 = [metrics[model][workload]["p95"] for metrics in confirmatory_metrics]
            confirmatory_median = statistics.median(confirmatory_p95)
            baseline_comparison[model][workload] = {
                "tier1_p95": baseline_p95,
                "tier1_raw_samples": tier1_metrics[model][workload]["samples"],
                "confirmatory_p95_by_replicate": confirmatory_p95,
                "confirmatory_raw_samples_by_replicate": [
                    metrics[model][workload]["samples"] for metrics in confirmatory_metrics
                ],
                "confirmatory_p95_median": confirmatory_median,
                "confirmatory_minus_tier1_percent": (confirmatory_median - baseline_p95) / baseline_p95 * 100.0,
            }

    baseline_sync_gap = {
        workload: {
            "change_percent": baseline_comparison["sync"][workload]["confirmatory_minus_tier1_percent"],
            "tier1_is_at_least_25_percent_lower": baseline_comparison["sync"][workload]["confirmatory_minus_tier1_percent"] >= 25.0,
        }
        for workload in WORKLOADS
    }

    affinity_effects = {}
    for workload in WORKLOADS:
        values = []
        stability = []
        for order in ORDERS:
            pinned_summary = lookup(probes, "pinned", "native", order, "sync", workload)
            unpinned_summary = lookup(probes, "unpinned", "native", order, "sync", workload)
            pinned = pinned_summary["p95"]
            unpinned = unpinned_summary["p95"]
            values.append((pinned - unpinned) / unpinned * 100.0)
            stability.extend(
                [
                    pinned_summary["rps_sample_relative_range_percent"],
                    unpinned_summary["rps_sample_relative_range_percent"],
                ]
            )
        affinity_effects[workload] = driver_report(values, 25.0, sample_stability=stability)
    affinity_supported = all(
        baseline_sync_gap[workload]["tier1_is_at_least_25_percent_lower"] and affinity_effects[workload]["supported"]
        for workload in WORKLOADS
    )

    order_effects: dict[str, Any] = {}
    for condition in CONDITIONS:
        order_effects[condition] = {}
        for workload in WORKLOADS:
            values = [lookup(probes, condition, "native", order, "sync", workload)["p95"] for order in ORDERS]
            stability = [
                lookup(probes, condition, "native", order, "sync", workload)["rps_sample_relative_range_percent"]
                for order in ORDERS
            ]
            order_effects[condition][workload] = {
                "p95_by_order": values,
                "relative_range_percent": relative_range(values),
                "sample_stability_relative_ranges_percent": stability,
                "sample_stability_within_10_percent": all(value <= 10.0 for value in stability),
            }
    order_supported = any(
        all(
            order_effects[condition][workload]["relative_range_percent"] >= 25.0
            and order_effects[condition][workload]["sample_stability_within_10_percent"]
            for workload in WORKLOADS
        )
        for condition in CONDITIONS
    )

    concurrency_effects: dict[str, Any] = {}
    for workload in WORKLOADS:
        native_benefit = []
        matched_benefit = []
        delta = []
        stability = []
        for order in ORDERS:
            native_sync_summary = lookup(probes, "pinned", "native", order, "sync", workload)
            native_hybrid_summary = lookup(probes, "pinned", "native", order, "hybrid", workload)
            matched_sync_summary = lookup(probes, "pinned", "matched", order, "sync", workload)
            matched_hybrid_summary = lookup(probes, "pinned", "matched", order, "hybrid", workload)
            native_sync = native_sync_summary["p95"]
            native_hybrid = native_hybrid_summary["p95"]
            matched_sync = matched_sync_summary["p95"]
            matched_hybrid = matched_hybrid_summary["p95"]
            native = (native_hybrid - native_sync) / native_sync * 100.0
            matched = (matched_hybrid - matched_sync) / matched_sync * 100.0
            native_benefit.append(native)
            matched_benefit.append(matched)
            delta.append(native - matched)
            stability.extend(
                [
                    native_sync_summary["rps_sample_relative_range_percent"],
                    native_hybrid_summary["rps_sample_relative_range_percent"],
                    matched_sync_summary["rps_sample_relative_range_percent"],
                    matched_hybrid_summary["rps_sample_relative_range_percent"],
                ]
            )
        concurrency_effects[workload] = {
            "native_benefit_percent": native_benefit,
            "matched_benefit_percent": matched_benefit,
            "native_minus_matched_percentage_points": driver_report(delta, 25.0, sample_stability=stability),
        }
    concurrency_supported = all(
        concurrency_effects[workload]["native_minus_matched_percentage_points"]["supported"]
        for workload in WORKLOADS
    )

    supported_drivers = []
    if affinity_supported:
        supported_drivers.append("environment/affinity")
    if order_supported:
        supported_drivers.append("ordering/warmup")
    if concurrency_supported:
        supported_drivers.append("client-concurrency fairness")
    classified_driver = supported_drivers[0] if supported_drivers else "unresolved"
    discrepancy_explained = classified_driver != "unresolved"
    if discrepancy_explained:
        recommendation = "sync default; revise the F4 measurement protocol before any new full selection run"
        protocol_revision = {
            "pinned_cpu_affinity": True,
            "equal_client_concurrency_accounting": True,
            "record_model_order": True,
            "record_elapsed_and_active_concurrency": True,
        }
    else:
        recommendation = "unresolved; retain sync default and close runtime selection as inconclusive"
        protocol_revision = None

    result = {
        "schema": "f4-hybrid-audit-v1",
        "experiment": "F4",
        "run_id": "foundation-f4-hybrid-audit-20260818",
        "profile": "Diagnostic",
        "inputs": {
            "tier1": str(args.tier1),
            "confirmatory": str(args.confirmatory),
            "probes": str(args.probes),
        },
        "validation": {
            "probe_cells": len(probes),
            "conditions": list(CONDITIONS),
            "client_modes": list(CLIENT_MODES),
            "orders": {str(index): list(order) for index, order in MODEL_ORDERS.items()},
            "models_per_probe": 3,
            "workloads_per_model": 2,
            "warmups_per_workload": 2,
            "samples_per_workload": 10,
            "service_requests": 64,
            "remote_requests": 32,
            "all_errors_zero": True,
        },
        "baseline_comparison": baseline_comparison,
        "baseline_sync_gap": baseline_sync_gap,
        "preserved_input_hashes": {
            "tier1": sha256_files(args.tier1),
            "confirmatory": sha256_files(args.confirmatory),
        },
        "normalization": {
            "timing_fields_excluded": sorted(DIAGNOSTIC_TIMING_FIELDS),
            "host_fields_excluded": ["captured_at_utc", "load_average"],
            "normalized_probe_outputs_are_included": True,
        },
        "probes": probes,
        "driver_evidence": {
            "environment_affinity": {
                "effects_pinned_minus_unpinned_percent": affinity_effects,
                "supported": affinity_supported,
            },
            "ordering_warmup": {
                "sync_p95_by_order": order_effects,
                "supported": order_supported,
            },
            "client_concurrency_fairness": {
                "effects": concurrency_effects,
                "supported": concurrency_supported,
            },
        },
        "supported_drivers": supported_drivers,
        "classified_driver": classified_driver,
        "discrepancy_explained": discrepancy_explained,
        "recommendation": recommendation,
        "protocol_revision": protocol_revision,
        "limitations": [
            "The original Tier-1 run has no host metadata, so its affinity and load cannot be reconstructed.",
            "Diagnostic probes are evidence about the loopback workload only and are not a new full F4 selection run.",
            "No hybrid or globally async architecture decision is made by this audit.",
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "classified_driver": classified_driver, "discrepancy_explained": discrepancy_explained}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
