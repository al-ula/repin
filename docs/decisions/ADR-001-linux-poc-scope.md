# ADR-001: Linux x86_64/glibc is the current PoC qualification scope

```text
Status: accepted scope decision
Date: 2026-08-19
Decision type: platform scope
Supersedes: none
```

## Decision

The current qualification and implementation target is Linux x86_64 with glibc.
The Linux PoC must carry the complete deterministic experiment matrix,
conformance checks, recovery evidence, and release-artifact smoke checks.

macOS, Windows, Linux musl/static builds, and additional architectures are
post-PoC expansion work. Linux evidence must not be presented as support for
those targets.

## Evidence

The prior foundation, follow-up, F4, F7, and F8 reviews all scoped their runs
to Linux x86_64/glibc. The reviewed results contained no non-Linux run.

## Consequences

- Platform-specific behavior remains behind adapters and explicit capability
  outcomes.
- The first implementation can optimize its evidence and release checks for
  one target without implying broader support.
- Platform expansion requires a separately scoped experiment and support
  policy; it does not reopen every Linux result automatically.

## Not decided

This ADR does not select a minimum Linux distribution, kernel, glibc version,
artifact format, signing policy, or eventual cross-platform support matrix.
