# Extraction

How files become facts. This is the layer where language support lives, and the layer most likely to be reimplemented, so its boundary is specified tightly.

## 1. Pipeline

```text
file
 ↓  detect         language and artifact class
 ↓  parse          syntax tree or structured form
 ↓  extract        local facts, self-contained
 ↓  resolve        cross-file references
nodes + edges
```

The split between **extract** and **resolve** is the important one. Extraction sees one file and nothing else. Resolution sees the graph. Keeping them separate is what makes extraction parallelizable, cacheable, and testable in isolation — and what allows resolution to be re-run without re-parsing.

## 2. Language pack

All language support arrives through one contract. No language is privileged, and the engine core contains no language-specific logic.

```text
LanguagePack
  id:            LanguageId
  version:       Version
  detects()   -> DetectionRule[]
  extract(input: SourceFile) -> FactBatch
  resolve?(request: ResolutionRequest) -> ResolutionResult
  capabilities() -> PackCapabilities
```

```text
PackCapabilities
  symbols:     bool     // can locate declarations
  references:  bool     // can locate uses
  imports:     bool     // can describe module relationships
  types:       bool     // can relate entities to type entities
  documents:   bool     // can extract prose structure
  nodeKinds:   NodeKind[]
  edgeKinds:   EdgeKind[]
```

A pack declares what it can do; it is not required to do everything. A pack providing only `symbols` is useful and supported. Capability declarations flow into the engine's own capability report ([Host Integration §2](host-integration.md#2-capability-negotiation)), so a caller can discover that a language is indexed but not traced.

Packs are engine components. **A repository can never supply a pack, a query, or any executable extraction logic** ([Safety and Data Handling §4](safety.md#4-no-execution)).

### Detection

```text
DetectionRule
  = ByExtension  { extensions: Text[] }
  | ByFilename   { names: Text[] }
  | ByShebang    { patterns: Text[] }
  | ByContent    { probe: ContentProbe }
```

Detection is deterministic and cheap. Ambiguity resolves by rule specificity, then by pack priority, then by stable pack ordering. Undetected files are still indexed as `file` nodes and remain findable by text search — an unsupported language degrades to text-only, never to invisible.

### Built-in Packs

- **`rust_pack`**: Extracts Rust source (`.rs`) into `struct`, `enum`, `trait`, `function`, `method`, `module`, doc summaries, and `UnresolvedRef` import dependencies.
- **`ts_pack`**: Extracts TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`) into `class`, `interface`, `type`, `enum`, `function`, `method`, JSDoc summaries, and module imports.
- **`py_pack`**: Extracts Python source and stubs (`.py`, `.pyi`, `.pyw`) into `class`, `function`, `method`, `variable`, docstring summaries, and `import` / `from ... import` dependencies.
- **`go_pack`**: Extracts Go source (`.go`) into `package`, `struct`, `interface`, `type`, `function`, `method`, `constant`, `variable`, doc summaries, and module import dependencies.
- **`c_pack`**: Extracts C source and headers (`.c`, `.h`) into `struct`, `enum`, `type`, `function`, `constant`, `variable`, `field`, doc summaries, `#include` imports, and call dependencies.
- **`cpp_pack`**: Extracts C++ source and headers (`.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx`) into `namespace`, `class`, `struct`, `enum`, `type`, `function`, `method`, `constructor`, `field`, doc summaries, inheritance, `#include` / `using` imports, and call dependencies.
- **`java_pack`**: Extracts Java source (`.java`) into `package`, `class`, `interface`, `enum`, `record`, `constructor`, `method`, `constant`, `field`, doc summaries, inheritance (`extends` / `implements`), `import` dependencies, and call / instantiation dependencies.
- **`csharp_pack`**: Extracts C# source (`.cs`) into `namespace`, `class`, `interface`, `struct`, `enum`, `delegate`, `constructor`, `method`, `property`, `constant`, `field`, XML doc summaries, inheritance (`extends` / `implements`), `using` imports, and call / instantiation dependencies.
- **`prose_pack`**: Extracts Markdown (`.md`, `.markdown`) into document, section, and heading structural hierarchy.

## 3. Extractor contract

```text
extract(input: SourceFile) -> FactBatch

SourceFile
  root:     RootId
  path:     Path
  bytes:    Bytes
  language: LanguageId

FactBatch
  nodes:      Node[]
  edges:      Edge[]
  unresolved: UnresolvedRef[]
  skips:      Skip[]
  diagnostics: Diagnostic[]
```

Hard requirements:

1. **Pure.** No IO, no graph access, no store handle, no shared mutable state, no network, no clock dependence.
2. **Batch in, batch out.** One call yields all facts for one file. Not a stream, not a callback, not a visitor the engine drives.
3. **Deterministic.** Identical input yields identical output, byte for byte, including ordering.
4. **Self-contained.** Facts reference other files only through `UnresolvedRef`. An extractor never asks "does this exist?"
5. **Bounded.** Respects time, memory, and fact-count limits; fails as a recorded skip rather than by hanging or crashing.
6. **No handle escape.** Parser-owned structures are released before returning. Nothing in `FactBatch` points into parser memory.

These are not stylistic preferences. Together they are what make extraction parallelizable across workers, cacheable by content hash, replaceable by a different implementation (including one in another language or another process), and testable against a fixture with no engine present.

### Coarse-grained traversal

**An extractor MUST NOT walk a parse tree node-by-node across a foreign-function boundary.**

Where the parser is a native or out-of-process component, per-node access dominates cost — often by an order of magnitude over parsing itself, and the cost is invisible in profiles that only measure parse time. Extraction must use whatever batch mechanism the parser offers: a pattern-match query API returning all captures in one call, a bulk serialized form, or a single traversal performed inside the parser.

Where a batch mechanism cannot express something, extract the minimal enclosing region with a batch query and do bounded local work inside it. The rule is one boundary crossing per file per query set, not per syntax node.

This constraint is stated portably because it is a property of foreign-function boundaries in general, not of any one parser or host language. An implementation with an in-process parser and no boundary crossing may ignore it. The concrete measurement that motivated it lives in the implementation profile, not here.

### Declarative extraction

Where the parser supports pattern matching, per-language extraction SHOULD be expressed as **declarative pattern files** rather than imperative code.

- Patterns are data: reviewable, diffable, testable, and portable across implementations of the same pack.
- Patterns are versioned with the grammar they target.
- Adding a language becomes writing patterns, not writing a traversal.

Imperative extraction is permitted where patterns cannot express the requirement, but it should be the exception and the reason should be recorded.

## 4. Prose and structured data

Source code is not privileged. Documentation, manifests, schemas, and configuration are first-class content, extracted by packs like any other language.

**Prose** yields `document`, `section`, and `heading` nodes forming a `contains` hierarchy, plus `links_to` edges for internal and external links and `mentions` for references to named entities. Section hierarchy matters: it is what allows retrieval to return the relevant part of a long document instead of the whole file.

**Manifests** yield `package` nodes and `depends_on` edges, and are usually `global` blast radius ([Incremental Updates §6](incremental.md#6-invalidation)) because they change resolution rules.

**Schemas** yield `schema`, `schema_field`, `table`, and `column` nodes with `validates_with` and `queries` edges to code where determinable.

**Configuration** yields `config_key` nodes. Configuration keys are frequently the thing a caller is looking for when tracing behavior, and they are invisible to code-only indexing.

A repository's prose and configuration often explain more than its source does. A pack set that only handles code produces a graph that answers "what calls this" but not "why does this exist."

## 5. Resolution

Resolution runs after extraction, with graph access.

```text
resolve(request: ResolutionRequest) -> ResolutionResult

ResolutionRequest
  refs:      UnresolvedRef[]
  scope:     ResolutionScope     // candidate definitions available
  rules:     ResolutionRules     // module mapping, aliases, search paths
```

Each reference resolves to exactly one outcome:

| Outcome | Meaning | Recorded as |
|---|---|---|
| resolved | bound to a definite target | edge, `derivation: resolved` |
| heuristic | bound by a fallible rule | edge, `derivation: heuristic` |
| external | target is outside the roots | edge to `external_symbol` |
| unresolved | no target found | `UnresolvedRef` retained |

Rules:

- Resolution is **deterministic**: same graph and same rules yield the same outcome. Where several candidates match, the tie-break is specified and stable, never "first found" over an unordered collection.
- Unresolved is a **normal outcome**, not a failure. Dynamic constructs, generated code, and unindexed dependencies legitimately do not resolve.
- Heuristic resolution MUST be marked heuristic. A name-matched call with an unknown receiver type is a guess, and downstream trust decisions depend on knowing that.
- Resolution never reads files. It works from extracted facts and configured rules. This is what allows it to re-run cheaply after a rules change without re-parsing.
- Resolution rules are versioned; changing them invalidates resolution-derived edges.

## 6. Versioning

Extraction output depends on things other than file content, so version records must capture all of them.

```text
grammarVersion      per language
packVersion         per language pack
patternVersion      per pattern set
extractorVersion    per extractor
resolutionVersion   resolution rules
classificationVersion  artifact-class rules
```

Every fact's provenance carries the extractor and version that produced it ([Graph Model §3](graph-model.md#3-provenance)). A version change invalidates **only the facts that extractor owns**, so upgrading one language pack re-extracts that language rather than rebuilding the graph.

Two operational consequences:

- A pack upgrade requires re-running golden snapshots ([Conformance](conformance.md)). Snapshot diffs are how grammar upgrades become reviewable instead of mysterious.
- **Different parser builds of the same grammar are not assumed equivalent.** Two builds nominally at the same grammar version can produce materially different node sets. Snapshots are per binding, and a fallback parser is never treated as a drop-in substitute for the primary one.

## 7. Parallelism

Extraction is embarrassingly parallel because the extractor contract makes it so.

```text
                coordinator
        ┌───────────┼───────────┐
     worker      worker      worker
        └───────────┼───────────┘
                    ↓
              FactBatch[]
                    ↓
                 resolver
                    ↓
          transactional writer
```

Rules:

- Workers receive paths (or paths plus content) and return `FactBatch` values.
- **Workers hold no store handle** and perform no writes. Many small writes from many workers is slower than batched writes from one, and it makes transaction boundaries impossible to reason about.
- Parse structures never cross the worker boundary; only plain serializable facts do.
- Resolution is centralized. It needs global visibility and does not parallelize the way extraction does — and it is the phase most likely to dominate at scale, so it deserves its own measurement rather than being assumed cheap.
- Pool size defaults to physical parallelism, not logical thread count. Oversubscription measurably reduces throughput on parser-bound work.

## 8. Failure handling

A single bad file must never prevent the rest of the index from building.

- Parse failure: record a `parse_failed` skip, keep the `file` node so the file stays text-searchable, continue.
- Partial parse: extract what is available and record a diagnostic. Most real parsers recover well; partial facts beat none.
- Limit exceeded: record the specific skip reason, continue.
- Pack crash or timeout: isolate, record, continue. A pack failure degrades one language, never the engine.
- Invalid encoding: replace invalid sequences for text purposes, flag the file, continue.

Every failure is queryable through skips and diagnostics. This is what keeps coverage honest — and coverage is what tells a caller whether an empty answer means anything.
