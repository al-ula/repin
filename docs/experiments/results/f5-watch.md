# Experiment Result: F5 — Watch adapter

```text
Status: deferred
Lifecycle stage: planning
Experiment specification: ../rust-foundation.md#6-f5-watch-adapter-notify-deferred-until-i3
Overall outcome: intentionally not run
```

## Result

F5 has no retained run by design. Watching is deferred until the I3 planning
milestone; explicit notification and polling remain complete alternatives in
the meantime.

## Recommendation

Keep `notify` unpinned and do not make a watcher a prerequisite for correctness
or freshness. Reopen F5 when I3 starts, record backend versions and overflow
semantics on Linux, and defer additional-platform event cases until the
post-PoC expansion phase.

## Required evidence at reopen

- platform events normalize to the documented change model;
- overflow and watcher loss force a bounded reconciliation scan;
- symlink, rename, and root-relocation cases preserve containment; and
- explicit notification remains equivalent to watcher-driven updates.
