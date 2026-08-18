#!/usr/bin/env python3
"""Unit tests for the disposable F4 hybrid-benefit audit helper."""

from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
import audit_f4_hybrid as audit  # noqa: E402


def diagnostic_fixture() -> dict:
    sample = {
        "sample": 0,
        "elapsed_us": 1000.0,
        "rps": 64_000.0,
        "expected_requests": 64,
        "client_completed": 64,
        "server_completed": 64,
        "client_errors": 0,
        "server_errors": 0,
        "client_max_active": 4,
        "server_max_active": 4,
        "client_max_queue": 8,
        "server_max_queue": 8,
        "client_workers": 4,
        "server_workers": 4,
    }
    workload = {
        "workload": "service",
        "requests": 64,
        "delay_us": 2000,
        "warmups": 2,
        "samples": [sample],
        "rps_samples": [sample["rps"]],
        "p50": sample["rps"],
        "p95": sample["rps"],
        "max": sample["rps"],
    }
    model = {
        "model": "sync",
        "order_index": 0,
        "configured_thread_count": 4,
        "runtime_thread_count": 0,
        "server_worker_count": 4,
        "client_concurrency": 4,
        "workloads": [workload],
    }
    return {
        "schema": "f4-hybrid-diagnostic-v1",
        "experiment": "F4",
        "run_id": "test",
        "condition": "pinned",
        "client_mode": "matched",
        "order_index": 0,
        "model_order": ["sync", "hybrid", "async"],
        "process_affinity": [0, 1, 2, 3],
        "available_workers": 4,
        "server_queue_capacity": 8,
        "warmups": 2,
        "samples": 10,
        "service_requests": 64,
        "remote_requests": 32,
        "binary_size_bytes": 1,
        "clean_build_time_ms": 1,
        "models": [model],
    }


class AuditHelperTests(unittest.TestCase):
    def test_percentile_rounding(self) -> None:
        values = [float(value) for value in range(10, 101, 10)]
        self.assertEqual(audit.rust_percentile(values, 0.50), 60.0)
        self.assertEqual(audit.rust_percentile(values, 0.95), 100.0)

    def test_fixed_model_rotations(self) -> None:
        self.assertEqual(audit.MODEL_ORDERS[0], ("sync", "hybrid", "async"))
        self.assertEqual(audit.MODEL_ORDERS[1], ("hybrid", "async", "sync"))
        self.assertEqual(audit.MODEL_ORDERS[2], ("async", "sync", "hybrid"))

    def test_native_and_matched_concurrency(self) -> None:
        self.assertEqual(audit.EXPECTED_CLIENT_WORKERS["native"], {"sync": 4, "hybrid": 8, "async": 8})
        self.assertEqual(audit.EXPECTED_CLIENT_WORKERS["matched"], {"sync": 4, "hybrid": 4, "async": 4})

    def test_normalized_probe_excludes_timing_fields(self) -> None:
        first = diagnostic_fixture()
        second = copy.deepcopy(first)
        second["binary_size_bytes"] = 999999
        second["clean_build_time_ms"] = 999999
        sample = second["models"][0]["workloads"][0]["samples"][0]
        sample["elapsed_us"] = 2000.0
        sample["rps"] = 32000.0
        second["models"][0]["workloads"][0]["rps_samples"] = [32000.0]
        second["models"][0]["workloads"][0]["p50"] = 32000.0
        second["models"][0]["workloads"][0]["p95"] = 32000.0
        second["models"][0]["workloads"][0]["max"] = 32000.0
        self.assertEqual(audit.normalized_probe(first), audit.normalized_probe(second))


if __name__ == "__main__":
    unittest.main()
