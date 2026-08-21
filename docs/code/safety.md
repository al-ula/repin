# Safety and Data Handling

Enforced inside the engine at L1–L3. A client MAY add restrictions; it MUST NOT be required to add them for the engine to be safe.

This placement is a direct consequence of standalone operation. The engine runs from a CLI and as a service, where no client-side policy layer exists. Any rule that lives only in an adapter is a rule that does not apply half the time.

## 1. Path containment

- Every configured root is canonicalized once at activation, with symlinks resolved.
- Every path derived from a request, a file's content, a link, or an external provider is canonicalized and checked against the roots before use.
- Traversal sequences, absolute paths outside roots, and symlink targets escaping a root are **rejected**, not clamped or sanitized into something adjacent.
- Symlink cycles terminate by tracking visited real paths.
- A path that resolves inside a root at check time but outside at read time (a symlink swapped underneath) fails the read rather than following it.
- Output paths are always root-relative, with a root identifier. Absolute host paths are internal.

Rejection over correction is deliberate: a "corrected" path silently answers a different question than the one asked.

## 2. Exclusions

Two independent mechanisms that are frequently confused:

**Selection rules** decide what may be read at all. They are a safety boundary. Secret-bearing patterns, dependency directories, build outputs, and version-control internals are excluded here, in the engine's own defaults. Product composition adds private metadata directories such as `.repin` explicitly.

**Scope** decides what a given query looks at within what is readable. It is a filter, not a boundary.

A caller can widen scope. A caller cannot widen selection to reach an excluded path. Conflating the two is how an "include everything" option becomes a credential disclosure.

Excluded-by-default categories:

- credential and secret files: environment files and their variants, private keys, certificate stores, credential and token stores, cloud provider credential directories
- version-control internals
- dependency and vendor directories
- build outputs, caches, generated artifacts
- binary content, detected by content sniffing rather than extension alone

An excluded path that is explicitly requested returns `SCOPE_EXCLUDED`. It does not return `not_found`: pretending a file does not exist is a worse answer than declining to read it, because it invites the caller to create it.

## 3. Redaction

Applied to every preview, log line, error message, and diagnostic — not only to previews.

- Detect credential-shaped content by pattern: key material, bearer tokens, connection strings with embedded credentials, high-entropy assignments to secret-named keys.
- Redact the value, keep the shape. `token = <redacted>` is useful; deleting the line is not.
- Redaction runs at the output boundary, so a fact that entered the graph before a rule existed is still redacted on the way out.
- Redaction is one-way and lossy by design. Any "unredacted" mode is a separate, explicit, non-default capability.
- Never redact into something that looks like a plausible real value.

Redaction is a mitigation, not a guarantee. Exclusion is the primary control; redaction catches what exclusion missed.

## 4. No execution

The engine never executes repository content to answer a query.

- Build scripts, configuration formats with executable semantics, code-generation toolchains, plugin systems, and test harnesses are **parsed, not run**.
- No compiler, bundler, package manager, or language runtime is invoked on repository content.
- No content is evaluated, deserialized into executable form, or dynamically loaded.
- Language packs are engine components, not repository content. A repository cannot supply a language pack, an extraction query, or a code path.

This forgoes some accuracy. Dynamically constructed imports, generated symbols, and macro-expanded declarations will be missed or approximated. That is the correct trade: an engine that runs repository code to index it is a code-execution vector triggered by cloning.

## 5. Untrusted input

Everything from outside the engine is untrusted data:

- **File content.** A hostile file must not escape its extractor: bounded time and memory per file, no unbounded recursion, size caps, graceful failure recorded as a skip.
- **Instruction-like text.** Content that reads as directives to an automated consumer carries no authority. The engine never interprets file content as configuration, policy, or instruction. Downstream consumers must be able to rely on this — it is why previews are labeled with provenance.
- **Provider responses.** Paths, ranges, and previews from an external provider are validated exactly as internally generated ones: containment-checked, bounds-checked, redacted, size-limited. A provider is not more trusted for being configured.
- **Configuration.** Validated and version-checked. Unknown fields produce diagnostics, never silent reinterpretation.

## 6. Resource bounds

Every operation is bounded. Unbounded work is a denial-of-service vector reachable by an ordinary repository.

Per-file: maximum size, parse time, memory, and extracted fact count. Exceeding any records a skip with a reason.

Per-query: pattern complexity, result count, evidence per result, traversal depth and breadth, path count, response size, wall time.

Per-session: concurrent operations, queue depth, total memory, worker count.

Two rules about how limits behave:

- **Exceeding a limit degrades; it does not fail the whole operation.** A pathological file is skipped; the index still builds. A too-broad traversal returns bounded results marked truncated; it does not error.
- **Every skip is recorded with a reason and is queryable.** Silent skipping makes coverage reporting a lie, and coverage is what callers use to decide whether an absent answer means anything.

## 7. Data egress

The deterministic engine is fully offline. Optional capabilities may not be.

- Remote embedding, reranking, and generation providers **transmit repository content off the machine**. This is the single most consequential configuration choice in the system.
- Remote providers are opt-in per capability, never enabled by default, never enabled as a side effect of enabling something else.
- The configured endpoint and model identity are reported in status output, so an operator can always discover where content goes without reading configuration files.
- A remote provider is only usable when the project is trusted.
- Content sent to a provider passes through the same exclusion and redaction rules as returned output. Excluded content is never embedded.
- Local model providers are still providers: they get the same capability negotiation and the same reporting, minus the egress warning.

## 8. State on disk

- Durable graph state lives in the project's `.repin` directory, self-ignored
  so derived artifacts are never accidentally committed:

  ```text
  project/.repin/
    .gitignore
    graph.sqlite3
    writer.lock
  ```

- State is not world-readable by default. It contains structural information
  about the repository and may contain content snippets.
- The user-daemon socket and singleton lease are separate from project state.
  They live in a private per-user runtime directory, must be owned by the
  current OS user, and must not be followed through a symlink or reparse point.
- Overflow output is not written to side-channel files by default. Content the caller did not ask to persist should not become a file.
- Deleting `.repin` is a rebuild/reset operation only after the corresponding
  project context has unloaded. If active state disappears, is replaced, or
  changes physical identity, the daemon fails that context closed rather than
  continuing through a stale handle or path.

### State-directory permissions

The engine creates and verifies the state directory before opening graph or
derived indexes. The default policy is private to the current user:

- On Unix-like systems, the directory is created with mode `0700` and
  engine-created regular files with mode `0600`. Existing broader permissions
  are tightened when the current user owns the entries and the platform can
  verify the result.
- On Windows, the engine creates or repairs a DACL that grants access to the
  current user and required system service principals only, disables inherited
  broad access for engine-created entries, and verifies the effective ACL.
  POSIX mode bits are not treated as a Windows security proof.
- Project state directories, project lock files, derived-index directories,
  runtime directories, daemon sockets, and daemon leases must not be a
  symlink or reparse-point escape. If the engine cannot verify the object and
  its permissions, activation or daemon startup fails closed.
- Repair is limited to the state directory and entries created/owned by the
  engine. The engine never recursively changes permissions in the repository
  root or user-selected external directories.
- If permissions are too broad and cannot be repaired, ownership is wrong, or
  the filesystem does not expose a trustworthy permission check, activation
  refuses the state with `CAPABILITY_UNAVAILABLE` and structured detail
  `STATE_PERMISSIONS`. It does not silently open read-only, because reading
  state can disclose repository structure just as writing can corrupt it.
  When a project database itself is invalid or newer but its state path is
  safe, the runtime may attach degraded for direct retrieval; that is a
  `PROJECT_STATE_INVALID` or `PROJECT_STATE_NEWER` outcome, not a permission
  exception.
- Permission repair and refusal are observable diagnostics. A successful
  repair does not change graph revision; a refusal leaves the previous state
  untouched and deletion remains the documented rebuild recovery.

## 9. Failure posture

When a safety check cannot be completed, **deny**.

- Cannot canonicalize a path: reject it.
- Cannot determine whether a file is excluded: treat it as excluded.
- Cannot determine whether the project is trusted: treat it as untrusted.
- Cannot apply redaction: withhold the preview.

Fail-closed costs recall. Fail-open costs disclosure. Recall is recoverable.
