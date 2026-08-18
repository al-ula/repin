# Experiment Result: F7 — Test, fuzz, benchmark, and dependency toolchain

```text
Status: complete (Tier 1 plus Q-series follow-up evidence retained; overall inconclusive)
Lifecycle stage: experimentation
Experiment specification: ../rust-foundation.md#8-f7-test-fuzz-benchmark-and-dependency-toolchain
Run ID: foundation-tier1-20260818
Follow-up run ID: q-release-tools-20260818
Overall outcome: inconclusive
```

## Result

Snapshot normalization, the four-input fuzz smoke probe, the fixed-loop
benchmark probe, locked Cargo metadata, and the intentionally negative policy
fixture all ran in the original Tier-1 batch. The pinned workspace and native
parser dependencies are represented in the metadata output.

The Q-series follow-up then installed and ran the exact candidate tools:

- Q-003: `assert_cmd` 2.2.2 and `insta` 1.48.0 pass the reviewed normalized
  snapshot and black-box CLI tests with snapshot updates disabled.
- Q-006: the clean dependency graph passes `cargo-deny` 0.20.2; the GPL
  fixture and a local Git-source fixture both fail closed. Duplicate versions
  remain warnings and the full report is retained.
- Q-007: `cargo-audit` 0.22.2 reports a clean baseline; the isolated
  `time = 0.1.45` fixture reports RUSTSEC-2020-0071 and the policy wrapper
  blocks it. The exception file is empty and its validator/unit tests cover
  the required outcomes.
- Q-008: `cargo-sbom` 0.10.0 produces SPDX JSON 2.3 and CycloneDX JSON 1.6.
  The SPDX package set matches the 232-package normal/build-resolved
  metadata scope; the CycloneDX component set matches the corresponding 231
  records without the document root. Tree-sitter core/grammars are present.
  The isolated USearch fixture matches 13 SPDX records and contains `usearch`
  and `cxx`.
- Q-012: `cargo-auditable` 0.7.5 builds and `cargo audit bin` recovers both
  binary dependency inventories. All 17 positive/negative cases pass.

The native boundary is intentionally recorded conservatively: the SBOM shows
the owning Rust crates (`tree-sitter`/grammars and `usearch`/`cxx`); native C
or C++ source files are not emitted as separate Cargo components by this
workflow.

## Provisional recommendation (decision deferred)

Retain the quality, security, benchmark, and SBOM tools as provisional
candidates. Use SPDX JSON 2.3 as the provisional canonical SBOM and retain
CycloneDX JSON 1.6 for compatibility comparison. Use the documented Q-007
advisory policy as the provisional response process, with no ignore-list
exceptions. These are evidence-backed recommendations only; the decision
status remains deferred.

This recommendation is recorded for later plan finalization. The experiment
does not accept or reject a candidate or select an implementation default.

## Remaining follow-up

- add actual Criterion/iai-callgrind and bounded fuzz targets; and
- repeat the release-tool evidence on additional supported platforms;
- verify native component inventory with any future artifact format that can
  represent raw C/C++ inputs separately.

## Evidence

- [feature-run batch report](raw/foundation-tier1-features-20260818/batch.json)
- [feature-run F7 report](raw/foundation-tier1-features-20260818/F7-report.json)
- [locked workspace manifest](raw/foundation-tier1-features-20260818/manifest.json)
- [spike workspace](../foundation_spike/)
- [Q-series follow-up report](raw/q-release-tools-20260818/report.json)
- [Q-series follow-up manifest, hashes, and commands](raw/q-release-tools-20260818/manifest.json)
- [reviewed Q-003 snapshot](../foundation_spike/tests/snapshots/q003_quality_tools__q003_preflight_manifest.snap)
- [SPDX 2.3 baseline SBOM](raw/q-release-tools-20260818/artifacts/sbom-spike-spdx.json)
- [CycloneDX 1.6 compatibility SBOM](raw/q-release-tools-20260818/artifacts/sbom-spike-cyclonedx.json)
- [isolated USearch SPDX inventory](raw/q-release-tools-20260818/artifacts/sbom-usearch-spdx.json)
- [baseline advisory JSON](raw/q-release-tools-20260818/artifacts/audit-baseline.json)
- [RUSTSEC-2020-0071 advisory JSON](raw/q-release-tools-20260818/artifacts/audit-time-advisory.json)
- [auditable foundation-binary report](raw/q-release-tools-20260818/artifacts/auditable-spike-build-audit.txt)
- [auditable USearch-binary report](raw/q-release-tools-20260818/artifacts/auditable-usearch-build-audit.txt)
- [Q-007 policy](../advisory-policy.md)
