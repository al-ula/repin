# ADR-003: Use capability-relative filesystem opens

```text
Status: accepted contract decision
Date: 2026-08-19
Decision type: filesystem safety
Supersedes: none
```

## Decision

Filesystem reads that may be reached through repository discovery must be
reopened relative to a retained root capability, with no-follow or equivalent
revalidation at the final components. Traversal, absolute paths, escaping
symlinks, component swaps, and final-component swaps fail closed. The engine
never returns bytes observed outside the configured root.

The root-relative capability protocol is the decision; the exact Rust crate or
platform API remains a candidate selection.

## Evidence

F-008 rejected all 200 component/final swap attempts under the capability path
and returned no outside bytes. The baseline check-then-absolute-open path
returned outside-root bytes in both swap cases. F-018 reported passing outcomes
for the broader Linux path, cycle, depth, encoding, and reconciliation matrix.

## Consequences

- Canonicalize-then-open remains only a comparison baseline; it cannot
  satisfy the containment contract under the tested race.
- Discovery and read paths must report bounded omission or capability errors,
  never silently widen scope.
- Platform adapters may implement the protocol differently, but a platform
  that cannot provide the fail-closed behavior cannot be accepted by weakening
  this contract.

## Not decided

This ADR does not accept `cap-std`, `ignore`, or `globset`, and it does not claim
containment behavior on non-Linux platforms.
