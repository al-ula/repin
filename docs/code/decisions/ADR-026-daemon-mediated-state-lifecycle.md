# ADR-026: Daemon-mediated state lifecycle and fail-closed database identity

```text
Status: accepted contract decision
Date: 2026-08-21
Builds on: ADR-015, ADR-024
Backs: docs/runtime.md, docs/api.md, docs/conformance.md
```

## 1. Context

`docs/runtime.md` requires that project state creation and removal be
daemon-mediated, that a context own the `.repin/writer.lock` handle for its
entire lifetime, and that a context fail closed when its database changes
physical identity. The initial implementation performed `init` and `uninit`
entirely in the client process: the CLI created or deleted `.repin` directly
and never informed the daemon.

Because the context registry is keyed only by the canonical database path, an
unlink followed by a re-initialization at the same path reuses the previously
loaded context. The daemon then serves queries from, and commits writes to, an
unlinked inode while holding a writer lease on a deleted lock file. A freshly
initialized, empty database silently reports the previous graph. The registry
key is correct as an identity; the missing element is an active guard that
revalidates physical identity and a lifecycle operation that unloads a context
before its durable state is removed.

## 2. Decision

State lifecycle transitions are daemon operations, and physical database
identity is an active guard on every context lookup and project request.

### 2.1 Lifecycle operations

Two unbound post-bootstrap requests are added. Both are issued before project
binding, because a connection cannot be bound to a project whose state does
not yet exist or is about to be removed:

| Operation | Daemon obligations |
| --- | --- |
| initialize | create `.repin` with owner-only permissions, create the ignore marker, acquire the project writer lease, create `graph.sqlite3` under that lease, publish the context holding the same lease, and bind the requesting connection to it |
| uninitialize | resolve the state directory, unload that project's context (closing the store and releasing the writer lease), then remove the directory |

Creation order is normative. The writer lease is acquired before the database
is created, and the handle that guarded creation is the handle the published
context keeps for its lifetime. Acquiring a lease, releasing it, and
reacquiring it during activation is not conforming: the gap admits another
process between creation and publication. Where a project's lease is already
held externally, initialization does not steal it — the project is created or
attached as an observer per §6 of docs/runtime.md.

Initialization classifies state before reporting success. The store adapter
stamps its format and schema version at creation, and initialization surfaces a
creation or validation failure using the existing state vocabulary
(`PROJECT_STATE_INVALID`, `PROJECT_STATE_NEWER`) rather than reporting a
successful initialization of unusable state. A `graph.sqlite3` entry that is
not a regular file is not a marker and is `PROJECT_STATE_INVALID`. This closes
a reported gap in which a non-regular state entry produced a successful `init`
followed by `PROJECT_NOT_INITIALIZED` on the next command.

Initialization MUST NOT overwrite an existing database; an already-initialized
project binds the existing context instead. Uninitialization MUST NOT remove
durable state while any other connection is attached to that context: it
returns `PROJECT_LEASE_UNAVAILABLE` and leaves the state intact. A client that
finds no reachable daemon MAY remove an unattached state directory itself; it
MUST NOT start a daemon solely to uninitialize.

Both operations are idempotent in their observable outcome: initializing an
initialized project succeeds without recreating state, and uninitializing an
uninitialized project succeeds while reporting that nothing was removed.

### 2.2 Fail-closed identity guard

A context records the platform identity of its database when the store is
opened. That identity is revalidated at two points:

1. on every registry lookup for a canonical path, before a cached context is
   returned to a new connection;
2. before dispatching any project-bound domain request against a context.

If the database is missing or its identity differs from the recorded identity,
the context is marked closed and evicted from the registry. It is never
rebound to the new file. A lookup then performs a fresh activation cycle for
the current file; an in-flight request on a closed context returns
`PROJECT_STATE_INVALID` with the recovery category, and the client reconnects.
A closed context serves no further graph reads or writes.

Identity remains an active safety guard only, as already specified: it is
never exposed as a portable project identifier.

### 2.3 Protocol version

The added requests and responses extend the domain protocol, so the protocol
range advances to `2`. Range negotiation, bootstrap bounds, and conservative
replacement are unchanged and continue to follow ADR-024: a client advertising
a strictly newer range may replace only a fully idle daemon, and a busy
incompatible daemon returns actionable `PROTOCOL_MISMATCH`.

## 3. Consequences and validation gate

Deleting or replacing `.repin` out of band remains safe rather than silently
wrong: the next lookup or request observes an identity change and fails that
context closed. State removal through the daemon is deterministic, releases
the writer lease, and closes the store before the directory disappears.

Conformance must cover daemon-mediated initialization and uninitialization
(including the attached-client rejection and the no-daemon client fallback),
lease-before-create ordering with lease continuity into the published context,
state classification on initialization (including a non-regular `graph.sqlite3`
entry), identity-change eviction on lookup, `PROJECT_STATE_INVALID` on a closed
context, and the absence of graph results from a removed database after
re-initialization.
