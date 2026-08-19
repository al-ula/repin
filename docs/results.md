# Results and Evidence

Normalization contract for every value the engine returns. This is L3 in [Architecture](architecture.md).

The purpose of this layer is that **no client normalizes anything**. Two clients written independently against this contract must format the same answer the same way, and must be unable to accidentally present a stale fact as current.

## 1. Envelope

Every capability returns:

```text
Result<T>
  status:      Status
  data:        T
  warnings:    Warning[]
  provenance:  Provenance
  freshness:   Freshness
  truncation?: Truncation
```

### Status

```text
ok           the operation completed and the answer is whole
partial      the answer is usable but incomplete; warnings say why
not_found    the operation ran to completion and found nothing
unavailable  a required capability could not be reached
invalid      the request was well-formed but semantically wrong
```

`partial` is a success. The common case is a direct search that succeeded while an optional graph expansion failed. Returning `unavailable` there would discard usable results; returning `ok` would hide the gap.

`not_found` asserts something: that the engine searched the stated scope with the stated capabilities and there was nothing. It is not a substitute for `unavailable` or `invalid`.

### Warnings

```text
Warning
  code:    ErrorCode
  message: Text          // human-readable, redacted
  detail?: Structured    // machine-readable specifics
```

Warnings are how `partial` explains itself. A `partial` result with no warnings is malformed.

### Provenance

```text
Provenance
  sources:   Source[]           // working-tree | graph | semantic | enrichment
  providers: ProviderId[]       // which implementations contributed
```

A caller must be able to see that a result mixes current file evidence with derived graph facts. Mixed results are normal; unlabeled mixed results are not.

### Public provider identity

`ProviderId` is a stable, opaque public alias registered at the composition
root. It is not an internal crate name, executable path, host account, model
endpoint, process ID, or credential identifier. Internal `ExtractorId`,
resolver IDs, and enrichment producer IDs remain private graph ownership data;
the registration maps each contributing internal producer to one public alias
before a result crosses the API boundary.

Every configured producer that can contribute public evidence MUST have a
registration. An unregistered producer cannot be selected for a public result;
the engine reports a capability/configuration diagnostic instead of emitting a
made-up or private identifier. When several registered producers contribute to
one result, all aliases are retained, deduplicated, and sorted by their opaque
wire value. Provider versions remain separate metadata in `ProviderInfo` and
internal provenance; a version change does not silently change the provider's
logical ID.

The public registry exposes only the minimum descriptor needed for capability
negotiation and freshness reasoning:

```text
ProviderInfo
  id:        ProviderId
  kind:      parser | resolver | store | lexical | vector | model | host | other
  version?:  Version
  location:  local | remote | host_supplied
```

Display labels, endpoints, dependency graphs, and internal ownership details
are diagnostics/configuration concerns and are redacted or withheld under the
same safety rules as any other provider response. A provider response is still
untrusted input; its claimed identity never overrides the registered alias.

### Freshness

```text
Freshness
  observedAt?:     Timestamp      // when the working tree was read
  graphRevision?:  Revision       // opaque; see below
  graphState?:     current | stale | building | unknown
  lexicalRevision?: Revision
  lexicalState?:   current | bypassed_lagging | disabled | failing
  coverage?:       complete | partial | unknown
```

Rules:

- `graphState` MUST NOT be `current` unless the engine has positively established it. `unknown` is the honest default and is expected to be common.
- `coverage` answers "could this answer have been complete?" A `not_found` with `coverage: partial` is materially weaker than one with `coverage: complete`, and callers reason differently about them. Unresolved references ([Incremental Updates](incremental.md)) are a direct input to this field.
- `graphRevision` is **opaque to clients**. It may be compared for equality only. Clients MUST NOT order, arithmetic, or parse it. This lets an implementation change revision representation without breaking consumers, and lets a host pass a token back for efficient change queries without understanding it.
- A lagging lexical index is bypassed rather than queried. Current working-tree scans and graph channels may still produce complete results; in that case the result may remain `ok`/`not_found` with `lexicalState: bypassed_lagging` and a warning. If scan limits, skips, timeout, or unavailable current channels reduce coverage, the result is `partial` with `coverage: partial`. Lexical lag alone does not justify claiming partial coverage if the fallback fully searched the requested scope.

### Truncation

```text
Truncation
  truncated: bool
  returned:  Count
  available?: Count            // when cheaply known
  reason:    limit | bytes | lines | depth | breadth | provider | timeout
```

Silent truncation is a correctness bug. A caller that believes it received a complete answer will reason from a partial one, and for automated callers that produces confidently wrong conclusions.

## 2. Evidence

The atomic unit of traceability. Every factual claim carries evidence or explicitly states that it has none.

```text
Evidence
  root:        RootId          // which configured root
  path:        Path            // relative to that root, normalized
  range?:      Range
  preview?:    Text
  observedAt?: Timestamp
  contentHash?: Hash           // identifies the exact version seen

Hash
  algorithm: HashAlgorithm     // stable registered identifier, e.g. blake3
  digest:    Bytes             // raw digest bytes, algorithm-defined length
```

Rules:

1. Paths in output are always root-relative. Absolute host paths are an internal detail and MUST NOT appear in returned results.
2. Evidence resolving outside a configured root is **rejected**, not displayed. This applies to evidence supplied by an external provider as much as to internally generated evidence.
3. Previews are line-bounded, length-bounded, and pass through redaction ([Safety and Data Handling](safety.md)).
4. Missing evidence is `absent`. Never synthesize a plausible location. A fabricated line number is worse than no line number, because it invites a caller to act on it.
5. `contentHash` lets a caller detect that evidence describes a version of the file that no longer exists.
6. A hash is never an untagged digest. Persisted and public hashes carry a stable algorithm identifier and raw digest bytes. Text transports render an algorithm-prefixed form such as `blake3:<encoded-digest>` using a protocol-specified encoding; adapters MUST NOT infer an algorithm from digest length. Hash algorithms and text encodings are open registries, while equality requires both algorithm and digest to match.

### Ranges

```text
Range
  start: Position
  end:   Position

Position
  line:   Count      // 1-based
  column: Count      // 1-based, in characters
  offset?: Count     // 0-based, in bytes
```

Line and column are 1-based because they are shown to humans and consumed by editors. Byte offsets are 0-based and optional. Both may be present; when they disagree, the character position governs display and the byte offset governs slicing. See [Graph Model](graph-model.md) for encoding rules.

## 3. Entities and relationships

Client-facing projections of graph nodes and edges. Deliberately narrower than the internal model — internal identity components, attribute bags, and extractor bookkeeping do not cross this boundary.

```text
Entity
  id:          EntityId        // opaque, stable, engine-assigned
  name:        Text
  qualifiedName?: Text
  kind:        EntityKind
  artifactClass?: ArtifactClass
  language?:   LanguageId
  evidence:    Evidence[]
  source:      Source
  confidence?: Confidence
```

```text
Relationship
  from:       Entity
  relation:   RelationKind
  to:         Entity
  evidence:   Evidence[]
  confidence: Confidence
  derivation: extracted | resolved | heuristic | inferred
```

`derivation` is not decoration. An `inferred` relationship produced by an optional model layer MUST remain distinguishable from an `extracted` one produced by a parser, at every layer, forever. Flattening that distinction is how a heuristic becomes an unquestioned fact.

`EntityId` is opaque for the same reason `Revision` is: identity construction is internal ([Graph Model](graph-model.md)), and clients that parse IDs freeze the scheme.

## 4. Ambiguity

Resolution that finds several plausible answers MUST NOT choose silently.

```text
Resolution<T>
  = resolved   { entity: T }
  | ambiguous  { candidates: T[], truncated: bool }
  | not_found  { searched: Scope, coverage: Coverage }
```

Returning the best guess is the wrong default. A caller acting on a silently chosen candidate has no way to discover the mistake, whereas a caller handed candidates can disambiguate or ask. When candidates are numerous they are bounded and marked truncated.

## 5. Error taxonomy

Expected outcomes are statuses. Only genuine execution faults raise.

| Code | Meaning | Caller response |
|---|---|---|
| `INVALID_QUERY` | well-formed, semantically wrong | fix the request |
| `PATH_OUTSIDE_ROOT` | path or evidence escaped a root | fix the path |
| `SCOPE_EXCLUDED` | target intentionally excluded | widen scope or accept |
| `CAPABILITY_UNCONFIGURED` | never set up | configure, or stop asking |
| `CAPABILITY_UNAVAILABLE` | set up, unreachable now | retry later |
| `CAPABILITY_UNSUPPORTED` | reachable, cannot do this | do not retry |
| `STATE_PERMISSIONS` | engine state is too broad, unverifiable, or owned by another user | repair permissions or choose a private state directory |
| `PROJECT_NOT_INITIALIZED` | no complete `.repin/graph.sqlite3` marker was found for the selector | initialize the project or choose another root |
| `PROJECT_STATE_INVALID` | a project database exists but is invalid, corrupt, or unsupported | repair or rebuild explicitly; direct retrieval may remain available |
| `PROJECT_STATE_NEWER` | a project database schema is newer than this engine | use a compatible engine or rebuild explicitly; direct retrieval may remain available |
| `PROJECT_STATE_ALIAS` | an active database was addressed through another physical alias | use the existing context or stop it before reopening the alias |
| `PROJECT_LEASE_UNAVAILABLE` | another process owns the project's writer lock | use observer/direct retrieval or retry authoritative activation |
| `DAEMON_START_FAILED` | a detached daemon candidate failed before readiness | inspect bounded startup diagnostics and retry |
| `DAEMON_UNAVAILABLE` | no usable per-user daemon became reachable within the startup budget | retry or repair the private runtime directory |
| `PROTOCOL_MISMATCH` | client and daemon cannot negotiate a compatible protocol | install compatible client/daemon versions |
| `ENTITY_AMBIGUOUS` | several candidates | disambiguate |
| `ENTITY_NOT_FOUND` | searched, absent | check coverage |
| `NO_PATH_FOUND` | endpoints resolved, no path | check coverage |
| `STALE_RESULT` | served but known stale | refresh or accept |
| `INDEX_BUILDING` | index not yet usable | retry later |
| `REVISION_TOO_OLD` | change history compacted past or does not contain it | resync |
| `UPDATE_CONFLICT` | prepared update repeatedly became stale before commit | retry later; reconciliation is scheduled |
| `TIMEOUT` | deadline exceeded | retry or narrow |
| `CANCELLED` | caller aborted | none |
| `RESULT_TRUNCATED` | bounded by a limit | narrow or raise the limit; v1 has no general continuation token |

The three-way split of capability failure is the most important distinction here. `UNCONFIGURED`, `UNAVAILABLE`, and `UNSUPPORTED` demand configure / retry / stop respectively. An automated caller that cannot tell them apart will retry the unretryable indefinitely, which is the single most common way this kind of integration fails in practice.

Similarly, `ENTITY_NOT_FOUND` and `NO_PATH_FOUND` must never be produced without `coverage`. "I looked everywhere and there is nothing" and "I looked at part of it and found nothing" are different claims, and only one of them supports a conclusion.

## 6. Output shaping

Callers with bounded output need answers pre-shaped. Two independent limit tiers, applied in order:

**Semantic limits**, applied first, because they yield a coherent smaller answer:

```text
maxResults · maxEvidencePerResult · maxPreviewLines
maxDepth · maxBreadth · maxPaths
```

**Hard limits**, a backstop only:

```text
maxBytes · maxLines
```

Reaching a hard limit means the semantic limits were too loose; it SHOULD be recorded as a diagnostic, not treated as routine.

Shaping rules:

- Prefer fewer results with full evidence over many results with none. Evidence is what makes a result actionable.
- Order for consumption: status and counts first, then the highest-ranked material. A truncated response must still lead with its most useful content.
- Group naturally — by distance for impact, by path for direct hits, by relation for neighbors — so truncation cuts whole groups rather than fragments.
- Never write overflow to a side channel by default. Content the caller did not ask to persist should not become a file on disk.

### Pagination stance for v1

Search, neighbors, trace, impact, ambiguity candidates, diagnostics, and
context are bounded **non-pageable** operations in v1. A `limit` or budget may
produce `RESULT_TRUNCATED` and an honest `Truncation` record, but the envelope
contains no continuation token. A repeated request is a new query and may see
different working-tree or graph state; clients MUST NOT emulate pagination by
assuming that an offset remains stable.

Callers that need more material narrow filters, increase the limit within the
call budget, or rerun against a known revision where the operation supports
one. `changesSince` is the deliberate exception: its opaque revision token is
a synchronization protocol, not general result pagination. Adding pageable
results later requires a versioned continuation contract with a defined
snapshot/freshness guarantee; it cannot be inferred from the existing limit
field.

## 7. Stability

- Adding a status, code, or optional field is a minor change. Clients MUST tolerate unknown values in these positions rather than failing.
- Removing or repurposing one is breaking.
- `EntityId` and `Revision` are opaque: their internal format may change at any time, and no client may depend on it.
- The envelope shape itself is versioned with the [Public API](api.md).
