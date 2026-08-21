# Project Guidelines

## Specification-First Authority

The specification in `docs/` is the authoritative source of truth and normative blueprint for Repin.

- **Spec-First Rule**: Any modification to system architecture, data models, port contracts, wire protocols, subsystem algorithms, or significant design decisions **MUST** be formulated and updated in the normative specification (`docs/`) and/or recorded as an Architectural Decision Record (`docs/code/decisions/`) **prior** to code implementation.
- **Contract Fidelity**: Code implementations and test suites must strictly adhere to the specification. Code must never silently diverge from, bypass, or invent new semantics without updating the authoritative specification first.
- **Documentation Integrity**: Whenever changes affect architecture or design, ensure all cross-references remain valid, the table of contents (`docs/SUMMARY.md`) is maintained, and `mdbook build` compiles cleanly.
- **Usage-Doc Fidelity**: Any change touching the user-facing interface — CLI flags/commands, environment variables, or configuration files — **MUST** update the corresponding usage documentation. Code must never diverge from the documented interface without also updating the docs.

## Writing Style

- **Terse**: Say the minimum that conveys the meaning. No filler words, no restating the obvious.
- **No Litotes**: Do not use understatement or denial of the opposite to imply a point (e.g. avoid "not bad", "not unimportant"). State directly.
- **No Overcommenting**: Do not add comments that merely restate the code. Comment only for non-obvious intent, invariants, or rationale.

## Autonomous Goal Execution

When performing autonomous work for a `/goal`:
- Decompose the objective into explicit sub-goals and actionable tasks.
- For each task, strictly follow the cycle: **Plan** → **Evaluate** → **Implement** → **Review**
  1. **Plan**: Formulate the step-by-step approach, identify files/symbols involved, and clarify requirements against the authoritative specification.
  2. **Evaluate**: Assess against architecture invariants, port contracts, performance, and potential regressions.
  3. **Implement**: Perform concrete code edits, configuration changes, or test additions.
  4. **Review**: Verify implementation with tests, linting, conformance checks, and spec consistency before proceeding.
