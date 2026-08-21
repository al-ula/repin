# ADR-008: Fail-closed content checks and SPDX quality policy

```text
Status: accepted PoC policy; production release tooling deferred
Date: 2026-08-19
Decision type: content safety and release evidence
Supersedes: none
```

## Decision

For the Linux PoC, content detection uses the bounded in-house classifier:
explicit exclusions win, at most 8 KiB is inspected, and an uncertain result
returns a binary-or-diagnostic/skip outcome rather than failing open. Do not add
`infer` as a required dependency based on the prior corpus review.

SPDX JSON 2.3 is the canonical SBOM format for the initial profile. CycloneDX JSON 1.6
remains a compatibility comparison. The documented advisory policy remains
the accepted response process, with no ignore-list exceptions.

## Evidence

F-009's 11-row corpus gave the in-house classifier complete labeled coverage,
while `infer` classified only two rows and returned nine unknowns. F7 and Q-014
reported passing policy, advisory, SBOM, build-audit, and binary-inventory
checks, while explicitly deferring final tool selection.

## Consequences

- Unknown content is handled conservatively and remains observable as a
  diagnostic or skip reason.
- Release tooling can compare SPDX and CycloneDX outputs without treating
  either format as a product/runtime dependency.
- A larger corpus, additional platforms, or a changed release policy reopens
  this policy.

## Not decided

This ADR does not accept `infer`, `cargo-deny`, `cargo-audit`, `cargo-sbom`, or
`cargo-auditable` as final production release dependencies, and it does not define the final
release artifact signing policy.
