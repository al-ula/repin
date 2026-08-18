# Graph Model

The canonical internal representation. [Results and Evidence](results.md) describes the narrower projection clients see; this document describes what the engine stores and why.

Everything here is normative. Identity in particular is the foundation that incremental updates, change reporting, and the embedding cache all rest on, and getting it wrong invalidates those subsystems rather than merely slowing them.

## 1. Node

```text
Node
  id:             NodeId
  kind:           NodeKind
  name:           Text
  qualifiedName?: Text
  root:           RootId
  path:           Path
  range?:         Range
  language?:      LanguageId
  artifactClass?: ArtifactClass
  provenance:     Provenance
  attributes:     Attributes
```

`RootId` is a logical identifier for a configured root, not a hash of its
current absolute path. It is assigned by the caller/configuration and remains
stable when that root is explicitly rebound to a different path. The engine
stores paths relative to the `RootId`, so a relocation does not rewrite every
node and edge. A rebind still triggers root reconciliation before the graph is
reported current; physical directory identity and the old canonical path are
diagnostic evidence, never public identity components. Root IDs are unique
within one engine and two root specifications may not reuse an ID.

`name` is the entity's own identifier as written. `qualifiedName` is its fully-scoped form when the language has one; it is a display and search convenience, never an identity component.

Nodes carry `provenance` for the same reason edges do. Without it, a node inferred by an optional model layer is indistinguishable from one extracted by a parser, and enrichment cannot be selectively discarded and rebuilt. A model that puts provenance only on edges cannot answer "delete everything a model invented."

## 2. Edge

```text
Edge
  id:         EdgeId
  from:       NodeId
  to:         NodeId
  kind:       EdgeKind
  provenance: Provenance
  attributes: Attributes
```

Edges are directed. Direction is part of the kind's meaning: `calls` runs caller to callee, `contains` runs container to contained. Reverse traversal is a query concern, not a second edge.

## 3. Provenance

```text
Provenance
  root:             RootId
  path:             Path
  range?:           Range
  extractor:        ExtractorId
  extractorVersion: Version
  derivation:       Derivation
  confidence:       Confidence
  revision:         Revision
```

```text
Derivation = extracted | resolved | heuristic | inferred
```

| Derivation | Means | Example |
|---|---|---|
| `extracted` | read directly from a parse tree | a class declaration's name |
| `resolved` | derived by following a deterministic rule | an import bound to the file it names |
| `heuristic` | derived by a rule that can be wrong | a call bound by name when the receiver's type is unknown |
| `inferred` | produced by a model | a topic relationship from enrichment |

`confidence` is a graded value within a derivation, used by ranking. `derivation` is categorical and used for trust decisions. They are separate because a high-confidence heuristic is still a heuristic.

Two rules that hold everywhere and forever:

1. **`inferred` facts MUST remain distinguishable from the rest at every layer.** Flattening the distinction is how a guess becomes an unquestioned fact.
2. **`inferred` facts MUST be independently deletable and rebuildable** without touching deterministic facts. This is what makes optional intelligence genuinely optional.

### Ownership claims

Provenance also identifies a fact's owner claim:

```text
FactOwner = (root, path, producer, producerVersion)
producer = extractor | resolver | enrichment producer
```

- Extracted nodes/edges, skips, and extraction diagnostics are claimed by the extractor for the input file.
- An unresolved reference is claimed by the extractor/file that emitted the reference. Its promoted resolved or heuristic edge retains that referencing claim; the target definition does not own incoming edges.
- Resolver diagnostics derived from a reference are claimed by the resolver version plus the referencing file.
- Inferred facts are claimed by their enrichment producer and remain separate from deterministic claims.
- File-level crawl/read/classification skips and diagnostics use a reserved engine producer for that root/path.

Several claims may support the same canonical node or edge identity. Claims are stored independently; a deterministic materialization rule projects them into the canonical graph. Removing or invalidating one owner removes only that claim. The canonical fact disappears only when no valid claim remains. Conflicting claims are resolved by a versioned deterministic rule and diagnosed—never by store iteration order. This makes per-file replacement, extractor upgrades, resolution demotion, and enrichment deletion exact without deleting another producer's evidence.

`extractorVersion`/`producerVersion` participates in invalidation ([Storage §3](storage.md#3-version-records)): when a producer changes, only its claims are stale even though no file changed.

## 4. Identity

### Node identity

An ID is derived only from stable addressing components:

```text
NodeId = hash(kind, root, path, containerChain, name, discriminator)
```

- `containerChain` — enclosing named scopes, outermost first.
- `discriminator` — disambiguates identical siblings (overloads, repeated declarations) by ordinal among siblings that are otherwise indistinguishable.

**IDs MUST NOT incorporate line numbers, column numbers, byte offsets, or content hashes.**

This is the single most consequential rule in the model. An ID containing position means every edit churns every node below the edit point in the file: change reports become useless (everything is "changed"), incremental invalidation degenerates to whole-file replacement, and the embedding cache misses on every keystroke. An ID containing a content hash means an entity loses its identity when its body changes, which is exactly when you most want to track it.

Consequences:

- Moving an entity within the same named container in a file **preserves** its
  ID when its same-name sibling order and discriminator remain unchanged.
  Moving it across named containers changes `containerChain` and therefore
  changes the ID. Its `range` alone never participates in identity.
- Reformatting a file changes no IDs.
- Renaming an entity, or moving it to another file, is a delete plus an add. Rename detection is explicitly out of scope; a consumer that wants it can compare content hashes across a change report.
- Two same-named siblings depend on a deterministic ordinal assigned among
  siblings with the same kind, container chain, and name. The ordinal is
  assigned from canonical source order after adapter sorting; it is not a raw
  parser cursor or a byte offset. Reordering those siblings swaps their IDs.
  Inserting one before existing siblings shifts the following ordinals;
  inserting one after them leaves their IDs unchanged. This bounded churn is
  accepted: putting a position directly into the ID would churn every entity
  below an arbitrary edit and would be worse.

#### Same-name discriminator examples

Use `m` for a named module and `parse` for a function name. The suffixes below
are illustrative discriminator inputs, not public ID encodings:

```text
base:       m::parse [d0]  m::parse [d1]
append:     m::parse [d0]  m::parse [d1]  m::parse [d2]
prepend:    m::parse [d0]  m::parse [d1]  m::parse [d2]
            ^new          ^old d0        ^old d1
reordered:  m::parse [d0]  m::parse [d1]
            ^old d1       ^old d0
```

In the `append` case, both original IDs survive and only the new overload is
added. In the `prepend` case, the new declaration takes `d0`, so both original
declarations receive new IDs. In the `reordered` case, the two original IDs
exchange their ordinal assignments. Adding `format` before or between these
declarations does not change the `parse` ordinals because it is a different
name. Moving a declaration into another named container changes
`containerChain` and is a delete-plus-add even if its text is unchanged.

The same rule applies to same-name declarations whose language syntax calls
them overloads. A signature may be stored as an attribute and used for ranking,
but it is not silently substituted for the discriminator in v1. A language
pack that introduces a different stable discriminator must version that rule
and invalidate its affected claims. Anonymous entities retain the separate
`unstableId` behavior below; their ordinal churn is not a promise of stability.

### Anonymous entities

Some entities have no stable name: unnamed default exports, callback literals, anonymous classes. They are handled explicitly rather than by accident.

1. If the entity is addressable through a binding — an export declaration, an assigned variable — address it by that binding.
2. Otherwise use its ordinal position within the nearest named ancestor, and set `unstableId: true` in attributes.

`unstableId` nodes may be dropped and recreated on any edit to their file. They MUST NOT be embedding-cache keys, and consumers MUST NOT persist references to them across revisions. Treating them as stable produces cache corruption that is very hard to trace.

### Edge identity

```text
EdgeId = hash(from, to, kind, extractor)
```

Including `extractor` prevents two extractors that independently discover the same relationship from silently colliding into one fact with one provenance. Deduplication across extractors is then an explicit merge at commit or query time, with a defined winner, rather than an accident of hash collision.

### Opacity

`NodeId` and `EdgeId` are opaque outside the engine. The projection to `EntityId` ([Results and Evidence §3](results.md#3-entities-and-relationships)) may be the same value or a mapping; either way clients may compare for equality only. Any client that parses an ID freezes this scheme, and this scheme will change.

## 5. Kind registries

Kinds are an open but **registered** vocabulary. A kind not in the registry is a defect, not an extension point: unregistered kinds cannot be ranked, filtered, or rendered coherently.

### Node kinds

```text
structural   repository · root · directory · file
packaging    package · module · namespace
types        class · struct · interface · trait · enum · type · type_parameter
callables    function · method · constructor · property · accessor
values       variable · constant · field · parameter
prose        document · section · heading · link_target
data         schema · schema_field · table · column · migration
operational  endpoint · route · job · service · resource · config_key
external     external_symbol · external_package
derived      concept · topic · responsibility
```

`derived` kinds are only ever created by optional enrichment and always carry `derivation: inferred`.

### Edge kinds

```text
structure    contains · declares · defines
reference    references · calls · reads · writes · instantiates
typing       has_type · returns · accepts · implements · extends · constrains
modules      imports · exports · depends_on · resolves_to
prose        documents · links_to · mentions · anchors
data         queries · migrates · validates_with
operational  handles · configures · tested_by · deploys
derived      relates_to · summarizes
```

Every kind's registry entry declares: direction semantics, whether it may be inferred, valid endpoint kinds, and whether it is transitively meaningful (whether A→B→C implies a real A–C relationship, which traversal and impact analysis rely on).

**The registry MUST NOT assume all content is source code.** Prose, schema, and operational kinds are peers of code kinds, not afterthoughts. A repository's documentation and configuration frequently explain more than its source does.

## 6. Attributes

```text
Attributes = Map<Text, Value>
```

Open, but each node and edge kind declares its expected keys and value types in a central registry, versioned alongside the kind registry.

- Registered keys may be used for ranking, filtering, and display.
- Unregistered keys are preserved and returned, but MUST NOT influence ranking or filtering. Ranking on an unvalidated bag makes behavior depend on whichever extractor happened to write a key.
- Attributes are for facts about the entity (visibility, modifiers, arity, signature text, deprecation, doc summary). They are not a place to stash extractor bookkeeping; that belongs in provenance.

Common registered keys include `unstableId`, `visibility`, `modifiers`, `signature`, `docSummary`, `deprecated`, `generated`, `arity`.

## 7. Positions and encoding

Position handling is a common source of off-by-one and mojibake bugs across language boundaries, so it is specified rather than left to implementations.

```text
Position
  line:    Count      // 1-based
  column:  Count      // 1-based, counted in characters
  offset?: Count      // 0-based, counted in bytes
```

- Files are treated as UTF-8. Invalid sequences are replaced for text purposes and the file is flagged; they never abort extraction.
- `column` counts characters (Unicode scalar values), because that is what editors and humans use.
- `offset` counts bytes, because that is what slicing needs.
- Both may be present. When they disagree, character position governs display and byte offset governs slicing. A consumer that renders from a byte offset without decoding will produce broken output on non-ASCII lines.
- Line endings are normalized for line counting: `\r\n` and `\n` both count as one line ending. Byte offsets remain relative to the file's actual bytes.
- A range's `end` is exclusive.

## 8. Graph invariants

Checkable properties. A store violating any of them is corrupt, and [Conformance](conformance.md) asserts each.

1. Every edge endpoint references an existing node, **or** the reference is recorded as unresolved ([Incremental Updates §8](incremental.md#8-unresolved-references)). Dangling endpoints are never permitted.
2. Every node and edge carries provenance with a revision no greater than the current revision.
3. Every node's `root` is a configured root, and its `path` is contained within it.
4. Every kind appears in the kind registry.
5. `contains` forms a forest: each node has at most one container, and there are no cycles.
6. No two distinct nodes share an ID; no two distinct edges share an ID.
7. Every node is reachable from a root node through `contains`, except `external_*` nodes, which are deliberately unrooted.
8. Facts with `derivation: inferred` reference only nodes that exist deterministically or are themselves inferred; a deterministic fact never depends on an inferred one.

Invariant 8 is what guarantees that discarding all enrichment leaves a valid graph.

## 9. Deletion semantics

When a file is removed, its owned nodes are removed. References from elsewhere must not be left pointing at nothing.

- Edges whose endpoint no longer exists are **demoted to unresolved references**, retaining the name they were seeking, so a later re-add reconnects them ([Incremental Updates](incremental.md)).
- `external_symbol` represents a symbol that resolves *outside* the indexed roots — a dependency, a platform global. Missing-because-deleted is an unresolved reference, **not** an `external_symbol`. Conflating them makes a deleted local file look like a third-party dependency.
- Tombstones are retained only as long as the change-history window requires, then compacted.
- Vector entries for removed nodes are deleted in the same semantic-index transaction that observes the removal. Semantic lag may therefore surface stale *content*, but never a dangling node reference.

## 10. Projection to clients

The client-facing `Entity` and `Relationship` ([Results and Evidence §3](results.md#3-entities-and-relationships)) are deliberately narrower. The mapping:

| Internal | Client | Note |
|---|---|---|
| `NodeId` | `EntityId` | opaque either way |
| `kind` | `kind` | registry name, unchanged |
| `root` + `path` + `range` | `evidence[]` | becomes evidence, root-relative |
| `provenance.derivation` | `derivation` | passed through |
| `provenance.confidence` | `confidence` | passed through |
| `provenance.extractor` | — | internal |
| `provenance.revision` | `freshness.graphRevision` | envelope, not entity |
| unregistered `attributes` | — | withheld |

What is deliberately not exposed: identity components, extractor identity and version, container chains as structure, and unregistered attributes. All of these are internal degrees of freedom, and exposing them converts them into compatibility obligations.
