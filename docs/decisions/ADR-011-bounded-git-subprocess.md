# ADR-011: Use a bounded Git subprocess for initial VCS integration

```text
Status: accepted implementation choice for the initial Linux PoC
Date: 2026-08-19
Decision type: VCS adapter
Builds on: ADR-005
```

## Decision

Repin's initial `Vcs` adapter invokes the installed Git executable as a bounded
child process. It does not use a shell. The adapter parses stable,
machine-oriented, NUL-delimited output and normalizes it into the core-owned
changed-set, revision, and branch result types.

The initial command profile uses:

- porcelain-v2, NUL-delimited status output for staged, unstaged, untracked,
  conflicted, renamed, type-changed, and submodule state;
- NUL-delimited name/status diff output for changes between recorded and
  current revisions;
- explicit command flags for untracked, rename, submodule, and output behavior
  rather than relying on user presentation defaults; and
- separate bounded revision and branch-identity queries where required.

Every invocation has a fixed argument template, explicit working directory,
sanitized environment, closed or null standard input, bounded stdout and
stderr, a deadline, and kill-and-reap cancellation. Paths are treated as raw
records, validated against the selected project root, normalized, deduplicated,
and stably sorted before they enter the update pipeline.

If Git is absent, incompatible, produces malformed or excessive output, times
out, reports an unreachable recorded revision, or otherwise cannot provide a
complete changed set, the adapter returns the existing full-scan fallback
disposition. VCS failure never prevents a correct update.

## Rationale

Git documents porcelain status as a stable interface for scripts and recommends
the `-z` form for unquoted path records. Its diff interface likewise supports
NUL-delimited name/status records. Using the installed Git implementation gives
Repin the user's established worktree, index, submodule, shallow-history, and
repository-format behavior without linking a large Git reimplementation into
the initial binary.

Compared with `gix`, the subprocess profile:

- has no Rust VCS dependency graph;
- uses canonical Git behavior for the repository under inspection;
- isolates Git parser/repository failures from the daemon process; and
- is easy to abandon for the already-required full scan.

The costs are executable discovery, process startup, version variance,
environment hardening, output parsing, and child-process cancellation. Those
costs are bounded adapter responsibilities rather than reasons to weaken the
`Vcs` contract.

This selection is based on public Git and `gix` documentation plus theoretical
contract analysis. It does not claim a Repin-specific performance result.

## Security and process profile

The adapter must:

- execute an explicitly resolved `git` program directly, never through a
  command shell;
- prevent terminal prompts and credential interaction;
- disable optional locking and other unnecessary side effects;
- remove inherited variables that inject Git configuration, tracing, helpers,
  alternate object stores, or repository locations unless explicitly required
  by the adapter contract;
- override configuration-driven helpers that could execute external programs;
- cap stdout, stderr, record count, path length, and elapsed time;
- kill and reap the process tree on cancellation or deadline; and
- treat all output, exit status, paths, and repository metadata as untrusted.

Repository configuration that affects selection is not automatically trusted.
Repin's root containment and file-selection policy remains authoritative after
the changed set is parsed.

## Consequences

- Git is an optional runtime dependency for accelerated startup and branch
  change detection, not a correctness dependency.
- A machine without Git still supports full scanning, hashing, extraction, and
  retrieval.
- Process startup cost is accepted for startup and bulk-change detection; this
  adapter is not invoked per file.
- Git is not bundled by this ADR. Executable packaging, supported Git versions,
  and any redistribution obligations belong to release policy.
- `gix` remains a future alternative if Git-free operation, tighter in-process
  integration, or measured process overhead becomes a product requirement.

## Required implementation validation

1. Normalize staged, unstaged, untracked, deleted, renamed, type-changed,
   conflicted, ignored, and submodule cases without path corruption.
2. Cover filenames containing spaces, tabs, newlines, non-UTF-8 bytes, leading
   dashes, and other unusual but valid path bytes.
3. Cover detached HEAD, unborn branches, linked worktrees, shallow clones,
   submodules, rewritten history, and unreachable recorded revisions.
4. Demonstrate stdout/stderr overflow handling, malformed output rejection,
   timeout, cancellation, kill-and-reap behavior, and missing Git fallback.
5. Verify that repository configuration cannot trigger prompts or unauthorized
   helper execution through the accepted command profile.
6. Confirm that every incomplete or ambiguous result escalates to a full scan
   rather than being presented as a complete changed set.

## Reopen triggers

Reopen the selection if the subprocess cannot be hardened to the accepted
process and containment contract, supported environments commonly lack Git and
full scans are too costly, or measured spawn/version variance fails the runtime
budget. Evaluate a minimal-feature `gix` adapter against the same fixtures
before changing the `Vcs` port.

## Sources

- Git [status porcelain formats](https://git-scm.com/docs/git-status)
- Git [diff machine-readable formats](https://git-scm.com/docs/git-diff)
- Git [environment variables](https://git-scm.com/docs/git)
- `gix` [status and interruption API](https://docs.rs/gix/latest/gix/status/struct.Platform.html)
- gitoxide [development status](https://github.com/gitoxidelabs/gitoxide)
