# Runtime and IPC

This chapter is normative for process lifetime, local IPC, project discovery,
and project-context ownership. Repin runs as one on-demand daemon per
unprivileged OS user. The daemon is reached through one local socket and hosts
zero or more isolated project contexts in its process. Durable project state
remains in each project's `.repin` directory. This topology is accepted by
[ADR-015](decisions/ADR-015-hybrid-per-user-daemon-runtime.md).

There is no separate `ProjectId`. A project is addressed at runtime by the
canonical path to its `graph.sqlite3` file. The path is the registry key, not a
portable identifier that clients may persist or compare across machines.

## 1. Durable and runtime state

An initialized project has this minimum layout:

```text
project/.repin/
  graph.sqlite3
  writer.lock
```

The initialization marker is the pair of a `.repin` directory and a regular
`.repin/graph.sqlite3` file. The database's contents are validated after the
marker is found; validation determines graph capability and does not create a
second project identity. All project paths are subject to the permissions,
root-containment, and symlink/reparse checks in [Safety and Data Handling](safety.md).

The daemon's coordination state is separate from project state. On the Linux
x86_64/glibc PoC it is placed below the private per-user runtime directory,
for example:

```text
$XDG_RUNTIME_DIR/repin/
  daemon.sock       # pathname Unix-domain socket
  daemon.lock       # singleton lease held by the daemon process
```

The actual runtime-directory selection is platform-specific and is not part
of the project API. The directory MUST be private to the OS user, reject
symlink/reparse substitution, and use private permissions. Runtime files are
ephemeral coordination artifacts; deleting or recreating them MUST NOT delete
or reset a project's `.repin` state.

The two locks have different scopes:

| Artifact | Scope | Owner | Purpose |
|---|---|---|---|
| `daemon.lock` | one OS user | global daemon process | elect exactly one daemon candidate |
| `.repin/writer.lock` | one project database | that project's active context | serialize authoritative graph and derived-index writes |

Clients acquire neither lock.

## 2. Global daemon rendezvous

The normal client entrypoint is a project operation, not `open(EngineOptions)`.
The client first attempts to connect to the per-user socket. If no usable
daemon is listening, it starts a detached candidate from the same installed
binary and retries the socket with a bounded deadline.

Every candidate opens the private runtime directory and attempts the
OS-backed singleton lease. Exactly one candidate can hold `daemon.lock`. The
winner creates or replaces the listening socket only after acquiring the lease
and publishes readiness through the protocol handshake. Losing candidates
exit without serving requests; their clients reconnect to the winner.

A socket path alone is never proof that a daemon is alive. A client treats a
connect failure, malformed readiness response, or protocol incompatibility as
an unavailable endpoint. Stale socket cleanup is allowed only after the client
has failed to connect and a candidate has established that it owns the
singleton lease. A candidate MUST NOT unlink a live daemon's socket.

The daemon accepts bounded connections and frames. Each connection has a
bounded admission queue, negotiated protocol version, request identifiers,
deadlines, cancellation, and progress events. The daemon does not expose a
cross-project request surface; a domain connection is bound to one project in
its first project handshake.

The daemon exits on demand after its final project context unloads and no
bootstrap attempt or client connection remains. It closes the central socket
before releasing `daemon.lock`, and releases the lease last. A process crash
closes both the lease and all project lock handles through normal OS handle
release. A later client may repair stale socket state and recover projects
independently.

## 3. Project selection and discovery

The client selects a project with one of two forms:

```text
ProjectSelector
  = DiscoverFrom { path: Path }
  | AtRoot      { root: Path }
```

`DiscoverFrom` starts at the supplied working directory and walks toward the
filesystem root. The nearest ancestor containing both a `.repin` directory and
 a regular `.repin/graph.sqlite3` file wins. If the supplied path is a file, its
parent directory is the starting directory. The walk is finite, does not cross
the filesystem root, and reports `PROJECT_NOT_INITIALIZED` when no pair is
found.

An incomplete pair is not a project marker:

- `.repin/` without `graph.sqlite3` is incomplete; discovery continues upward.
- `graph.sqlite3` without its `.repin/` directory is incomplete; discovery
  continues upward.
- A non-regular, symlinked, or reparse-point state entry is not silently
  followed. Unsafe state is rejected or treated as incomplete according to
  the platform adapter, and an active alias is reported as
  `PROJECT_STATE_ALIAS`.

`AtRoot` bypasses ancestor selection and addresses exactly the supplied root.
It still canonicalizes and validates the state directory and still requires
the `.repin/graph.sqlite3` pair. Explicit selection therefore overrides which
ancestor is chosen, not the safety or initialization rules.

Before checking ancestors, the daemon canonicalizes the starting parent
directory, resolving parent-directory symlinks/reparse points. It then walks
the canonical physical parents, not the spelling supplied by the client. The
final `.repin` and `graph.sqlite3` components are opened with no-follow or an
equivalent revalidation guard. All selected paths are checked again before
activation so a directory replacement between discovery and open fails closed.

The canonical database path is computed as the canonical project root joined
to `.repin/graph.sqlite3` and then validated as a regular file. The active context
registry is keyed by this canonical path. Two clients selecting the same path
share one context; copying the database to another canonical path creates an
independent context, even when the copied contents are byte-for-byte equal.

Filesystem identity is an active safety guard only. While a context is loaded,
the daemon records the database identity needed by the platform adapter and
rejects an attempt to open the same underlying database through a symlink,
rename, bind mount, hard link, or alternate spelling. If an active database
disappears or changes physical identity, its context fails closed and is
marked closed; it is not silently rebound to a new file. Filesystem identity
is never exposed as a stable logical project identifier.

## 4. Initialization and graph capability

`repin init` is a daemon-mediated operation. It creates `.repin` with private
permissions, acquires that project's writer lock, and creates `graph.sqlite3`.
It MUST NOT overwrite an existing database. Creation and activation recheck
the canonical paths and filesystem identity before publishing the initialized
context.

Project membership and graph capability are separate outcomes:

| State | Connection outcome |
|---|---|
| No `.repin/graph.sqlite3` pair | `PROJECT_NOT_INITIALIZED`; choose another root or initialize |
| Pair exists and store validates | Attach normally; graph and available indexes may be used |
| Pair exists but is invalid, corrupt, or unsupported | Attach in degraded mode with `PROJECT_STATE_INVALID`; preserve bounded direct working-tree retrieval |
| Pair exists with a newer schema | Refuse graph access with `PROJECT_STATE_NEWER`; preserve safe direct retrieval |
| Pair resolves to an already-active physical database by another path | Reject the second attachment with `PROJECT_STATE_ALIAS` |
| Project writer lock is owned by another process | Attach an observer where safe; graph writes return `PROJECT_LEASE_UNAVAILABLE` |

An invalid or newer database is not silently rebuilt, replaced, or treated as
an uninitialized project. The result and status make graph unavailability
explicit. Direct retrieval still enforces the normal root, exclusion, bounds,
and redaction rules and does not depend on graph activation.

## 5. Bound connections and project contexts

The public project-bound operations are:

```text
initializeProject(spec: ProjectSpec, call?: CallOptions)
  -> Result<ProjectClient>

connectProject(selector: ProjectSelector, call?: CallOptions)
  -> Result<ProjectClient>
```

The first project handshake performs protocol negotiation, trust/capability
setup, and project selection or initialization. It binds the accepted
connection to exactly one project. After that handshake, domain requests omit
the project path. Request IDs permit concurrent operations, progress,
deadlines, and cancellation within the bound project. To change projects, a
client closes the connection and establishes another one.

Multiple connections may bind to the same canonical database path and share
the same warm context. They observe the same committed revision and status;
they do not each open a store or watcher. A connection bound to one project
cannot issue a request against another project's context.

Each active context independently owns:

- canonical root and project configuration;
- trust and capability state;
- the `.repin/writer.lock` handle, if it is the authoritative owner;
- graph storage and revision history;
- the watcher and update coordinator;
- lexical/vector indexes and pending derived work.

The registry lookup and context creation are atomic with respect to the
daemon's event loop. A second connection cannot create a second context for
the same canonical database path merely because the first is still loading.

`ProjectClient.close()` detaches that connection. It does not unload an active
context immediately, terminate the user-wide daemon, or cancel requests from
other connections. Requests belonging to the closed connection may be
canceled according to the transport contract; unrelated project and daemon
work continues.

`open(EngineOptions) -> Engine` remains an in-process composition API for the
daemon, deterministic tests, and explicit library embedding. It is not the
normal project-client entrypoint and does not by itself grant ownership of a
project lock or the daemon's sharing guarantees.

Implementation establishes this in-process engine and its port conformance
first, then adds the daemon composition root, context registry, and bounded
protocol. Multi-client sharing, watcher ownership, and the normal project-bound
CLI are conforming only through the completed daemon path; the sequencing does
not create a second public runtime topology.

## 6. Lock ownership and observer mode

When a context activates a writable project, the global daemon process holds
the OS-backed `.repin/writer.lock` handle for the entire authoritative
context lifetime. The lock's metadata, if any, is diagnostic only; metadata,
timestamps, PIDs, and deleting a lock file are never ownership proofs.

If another process already owns the project writer lock, the daemon does not
steal it. It may attach an observer context when the store can be opened
safely for reads. In observer mode:

- bounded direct working-tree retrieval remains available;
- validated graph reads may be served with explicit reader/observer status;
- graph writes, watcher commits, and derived-index mutations return
  `PROJECT_LEASE_UNAVAILABLE`;
- the daemon does not buffer mutations as if it were authoritative.

The daemon may retry activation after the external owner releases the lock,
but promotion to authoritative mode requires a fresh identity and store
validation check. Unsupported or unverifiable locking fails closed.

## 7. Context and daemon lifecycle

A context is **idle** when it has no attached clients, no in-flight requests,
no authoritative commit in progress, and no mandatory recovery work. Watcher
registration by itself is not activity. An idle context is unloaded after
`600,000 ms` (ten minutes), unless a new client attaches first.

Unloading stops the watcher, drains or records optional derived-index work,
closes graph and index stores, releases the project writer lock, and removes
the context from the canonical-path registry. It must not affect other
contexts, even when their projects share a parent directory or were opened by
the same client process.

The global daemon remains alive while any context is active, any client is
connected, or bootstrap/startup work is in progress. Once the final context
has unloaded and no bootstrap or client connection remains, the daemon stops
accepting work, closes the central socket, and releases the singleton lease
last. A client can then start a fresh daemon without relying on stale metadata
or a manual cleanup command.

Deleting `.repin` is a rebuild/reset operation only after its context has
unloaded. If deletion, replacement, rename, or a physical identity change is
observed while the context is active, the daemon fails that context closed and
requires a new discovery/activation cycle. It never continues writing through
a stale path.

## 8. Runtime errors

These errors are part of the public status/error vocabulary in addition to the
general taxonomy in [Results and Evidence](results.md):

| Code | Meaning |
|---|---|
| `PROJECT_NOT_INITIALIZED` | No valid `.repin/graph.sqlite3` pair was found for the selector. |
| `PROJECT_STATE_INVALID` | A database exists but is invalid, corrupt, or unsupported. |
| `PROJECT_STATE_NEWER` | The database schema is newer than this engine can read. |
| `PROJECT_STATE_ALIAS` | An active database was addressed through another physical alias. |
| `PROJECT_LEASE_UNAVAILABLE` | Another process owns the project's writer lock. |
| `DAEMON_START_FAILED` | A spawned daemon candidate failed before publishing readiness. |
| `DAEMON_UNAVAILABLE` | No usable daemon became reachable within the bounded startup/retry budget. |
| `PROTOCOL_MISMATCH` | Client and daemon cannot negotiate a compatible protocol version. |

Runtime errors MUST include a safe, actionable diagnostic without disclosing
credentials or unnecessary absolute paths. A state error MUST identify the
state class and the recovery category; it MUST NOT turn an invalid store into
an apparently empty graph.

## 9. Runtime invariants

The runtime implementation is conforming only if all of the following hold:

1. Concurrent cold-start clients produce one user daemon; losing candidates
   exit and reconnect.
2. Nearest-ancestor discovery selects the closest complete marker, while an
   explicit root overrides ancestor selection.
3. Two connections to one canonical database share a context and revision;
   two canonical database paths remain isolated even for copied contents.
4. An active physical alias cannot create a second context.
5. Invalid or newer graph state leaves bounded direct retrieval available with
   explicit graph-unavailable status.
6. Context eviction after ten idle minutes does not affect active contexts,
   and the daemon exits after the final context unloads.
7. Client termination detaches only its connection and cannot terminate the
   daemon or cancel unrelated requests.
8. Daemon death releases project locks; restart repairs stale rendezvous state
   and recovers each project independently.
9. Deadlines, cancellation, progress, and protocol negotiation work across a
   bound connection without reintroducing project paths into domain requests.

ADR-015 accepts the runtime topology. The invariants above and its named fault
cases remain required implementation validation and may reopen the decision if
the implementation evidence reaches an ADR-015 trigger.
