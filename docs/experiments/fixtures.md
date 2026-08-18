# Initial Fixture and Corpus Manifest

Planning input for extraction, convergence, safety, and benchmark experiments. This document fixes the fixture *shape* and size bands; exact public repositories and revisions are pinned by the preparation tasks below.

```text
Status: proposed
Lifecycle stage: planning
Normal target: workstation repositories up to 10,000 selected files / 250 MiB selected content
```

## 1. Initial extraction coverage

The first fixture family covers:

- Rust source
- Markdown documents
- TypeScript source
- JavaScript source, including the JS/TS module boundary

These are experiment inputs, not a commitment that every pack ships in the first deterministic implementation. Plan finalization uses extraction evidence to select initial shipped packs.

Each format has two complementary fixture sources:

1. **Repository-owned synthetic fixtures** provide exact expected facts, ranges, identities, unresolved references, diagnostics, and failure behavior.
2. **Pinned public-repository snapshots** provide representative syntax, repository layout, and workload evidence. Every snapshot records upstream URL, immutable commit, license, acquisition command, retained-file manifest, and content checksum. The experiment must remain reproducible if the upstream repository disappears; redistribution rules determine whether content or only an acquisition manifest is retained.

Public snapshots never replace exact synthetic oracles. Expected graph snapshots are reviewed independently of generated output.

## 2. Required synthetic fixture modules

### Rust

Cover modules and `use`, functions/methods, structs/enums/traits/impls, macros, attributes, nested items, same-name items in distinct containers, unresolved/external references, Unicode identifiers where accepted, comments/strings containing code-shaped text, malformed source, and `Cargo.toml` relationships where a separate manifest pack is evaluated.

### Markdown

Cover heading levels and repeated headings, sections, inline/reference links, local anchors, links across files, code fences, lists/tables, front matter as an explicitly classified extension, Unicode and combining marks, CRLF, very long sections, malformed links, and instruction-shaped prose with no authority.

### TypeScript and JavaScript

Cover ESM and CommonJS, imports/exports and re-exports, functions/classes/interfaces/types, overloads, JSX/TSX where in scope, dynamic imports, destructuring, type-only references, package aliases, same-name symbols, unresolved/external modules, comments/strings/templates containing code-shaped text, and tolerant parsing of incomplete source.

A mixed JS/TS fixture MUST verify cross-language module resolution and stable behavior when a `.js` implementation is consumed by TypeScript or emitted paths coexist with sources.

### Cross-format

Cover Markdown links to code, code symbols mentioned in prose, repository manifests/configuration, file create/modify/delete/rename, branch-like bulk replacement, whitespace-only edits, path relocation, and extraction-version invalidation.

## 3. Corpus size bands

Sizes refer to **selected content after default exclusions**, not every directory entry under a root. Byte values are MiB of selected file content. Node and edge counts are workload-generation targets used when a source corpus does not naturally hit the band; they are not product limits or performance promises.

| Band | Selected files | Selected bytes | Graph nodes | Graph edges | Purpose |
|---|---:|---:|---:|---:|---|
| Micro | 1–50 | up to 1 MiB | up to 5,000 | up to 15,000 | Exact golden snapshots, unit/port behavior, and failure minimization |
| Small | 51–1,000 | over 1 to 25 MiB | over 5,000 to 100,000 | over 15,000 to 300,000 | Routine integration, convergence, and developer feedback |
| Medium | 1,001–5,000 | over 25 to 125 MiB | over 100,000 to 500,000 | over 300,000 to 1,500,000 | Routine workstation repository and scaling evidence |
| Large normal | 5,001–10,000 | over 125 to 250 MiB | over 500,000 to 1,000,000 | over 1,500,000 to 3,000,000 | Initial largest normal target and release qualification |
| Pathological | Independently bounded per case | Independently bounded per case | Shape-specific | Shape-specific | Generated/vendor/dependency trees, giant files/lines, deep paths/nesting, high fan-in/out, binary disguises, and resource-limit behavior |

A corpus is classified by its highest file, byte, node, or edge band. Experiments report actual counts; they do not assume ratios from this table. Resolution, store, lexical, and end-to-end measurements must include the Large normal band before making claims about the initial target.

## 4. Selection and excluded content

Generated output, dependency directories, and vendor trees are excluded from the normal bands by default selection policy. They are exercised in separate pathological fixtures to verify:

- ignore and exclusion precedence
- explicit re-inclusion behavior where permitted
- bounded discovery and resource use
- omission diagnostics and coverage reporting
- no accidental indexing or provider egress
- convergence after exclusion/classification changes

The pathological corpus is not an unbounded stress test. Every case declares file, byte, depth, time, memory, and fact-count guards and records the expected bounded skip or degradation.

## 5. Fixture pinning and acceptance

Each fixture entry MUST include:

| Field | Requirement |
|---|---|
| ID and purpose | Stable identifier and linked contract/pass conditions |
| Content source | Synthetic generator revision or immutable public commit |
| License | SPDX identifier or recorded review; redistribution status explicit |
| Manifest | Sorted normalized paths, byte sizes, and content hashes |
| Oracle | Reviewed graph/range snapshot or stated measurement-only role |
| Versions | Grammar, query, pack, resolution, and classification versions |
| Scale | Actual files, bytes, nodes, edges, and relevant shape metrics |
| Safety class | Normal or named pathological case with resource guards |

A public fixture may be replaced only through a reviewed manifest change that explains lost/gained coverage. Benchmark history across different corpus revisions is not directly comparable.

## 6. Deterministic graph-shape generators

The store experiments in [Storage Adapter Experiments — Shared method](storage.md#2-shared-method) need graph *shapes* that a naturally occurring corpus does not reliably produce. These generators emit synthetic repository content plus an expected-fact oracle; they are fixture tooling, not product code, and they encode no accepted identity, resolution, or storage decision.

Common rules for every generator here:

- A run is fully determined by `(generatorName, generatorVersion, seed, parameters)`. Two Linux x86_64/glibc PoC runs of the same tuple must produce byte-identical files and an identical oracle. Portability and lower-tier reproducibility are post-PoC work.
- The PRNG is the ChaCha8 stream cipher (ChaCha20 construction reduced to 8 rounds, as implemented by `rand_chacha::ChaCha8Rng`) initialized with a 32-byte key of `u64le(seed)` followed by 24 zero bytes, a 12-byte all-zero nonce, and block counter 0. Its keystream is consumed as consecutive little-endian `u64` words in the documented construction order, single-threaded, even when files are written in parallel. A bounded value in `[0, n)` uses Lemire rejection over whole words: reject and redraw while `low < (2^64 mod n)`. A generator never draws for an empty domain, a single-candidate domain, or an allocation the parameters already determine, so adding a deterministic allocation cannot shift later draws.
- Manifests, parameter tuples, and oracles are emitted as JCS-canonical JSON ([RFC 8785](https://www.rfc-editor.org/rfc/rfc8785)) with LF and one trailing newline: UTF-8, keys sorted by UTF-16 code unit, minimal escaping, duplicate keys rejected, and array order significant and defined by the construction order below. To stay inside JCS's exact-integer range, every value that can exceed 2^53-1 — `seed`, byte offsets, and byte sizes — is emitted as a decimal **string**, and every hash or byte value is emitted as the string `{algorithm}:{lowercase hex}`. No floating-point value is ever emitted. Generated source files use LF and one trailing newline unless a case explicitly generates CRLF.
- Ordinals are zero-based and rendered as five zero-padded decimal digits, so a parameter above 100,000 is rejected rather than widened. A symbolic ID `hub{ordinal}` corresponds to emitted lookup text `hub_{ordinal}` with the same padded ordinal; `file`, `def`, `src`, `ref`, `missing`, and `alt_` texts follow the same rule.
- A percentage `p` applied to a count `n` yields `floor(n * p / 100)` items, assigned to the lowest ordinals first; remainders are never randomized. A parameter tuple whose allocations cannot all be satisfied is rejected with an error rather than silently adjusted.
- Expected facts use fixture-local symbolic IDs — `hub{ordinal}`, `src{ordinal}`, `file{ordinal}` — and the adapter under test maps them to its own identifiers. An oracle never assumes a production `NodeId`/`EdgeId` derivation.
- Oracle equality compares canonical topology and identity — nodes, edges, kinds, endpoints, unresolved references, and per-owner claim sets — and never the singular `Edge.provenance` range of a multi-occurrence edge, matching [Conformance — Graph equality](../conformance.md#graph-equality). “No canonical change” therefore means unchanged topology, identity, and remaining claims; a candidate may pick any occurrence as the canonical provenance representative, and the choice is recorded as an observation rather than a pass condition.
- Every generator validates its tuple against the cases it must support and fails loudly instead of producing a fixture that cannot exercise them: any owner-removal case requires `producersPerFile >= 2`, every fixture requires at least owner `v1`, any cross-file or `rename` case requires `fileCount >= 2`, any resolvable reference requires at least one definition, and every ordinal must fit the five-digit template.
- Multi-owner claims use **one** extractor identity `x1` with distinct extractor versions `v1..v{n}`. `FactOwner.producer` is that extractor identity and `FactOwner.producerVersion` is the version, so owners differ while `EdgeId = hash(from, to, kind, extractor)` from [Graph Model §4](../graph-model.md#edge-identity) stays equal and several claims legitimately support one canonical fact. A generator never invents a second extractor identity for a fact it expects to remain canonical-identical.
- An occurrence is fixture-side input, not necessarily a port-observable fact: a canonical `Edge` carries one optional provenance range, and portable [graph equality](../conformance.md#graph-equality) excludes occurrence counts. The oracle therefore always states expected canonical nodes/edges, and states per-occurrence expectations only for an adapter exposing a spike-local occurrence or claim-inspection hook (as the S1 sidecars in [Storage Adapter Experiments](storage.md#s1-experimental-key-encoding-s-002) do). A candidate without such a hook is measured against canonical counts and demotion behavior, and the missing visibility is recorded as a limitation rather than a failure.
- The generator writes a manifest of sorted normalized paths, byte sizes, and content hashes, plus the oracle, plus the exact parameter tuple, as required by [§5](#5-fixture-pinning-and-acceptance).
- Output paths, node names, and reference texts are generated from a fixed-width ordinal template (for example `f00042`), so sort order is stable across locales and platforms. Language or library default hashing/RNG, wall-clock time, thread scheduling, environment variables, and filesystem iteration order must not affect output.
- The oracle is derived from the generator's own construction plan, never from a store or extractor output. Any experiment that compares store results to the oracle must fail on both missing and extra facts.
- Every case declares its corpus band from [§3](#3-corpus-size-bands) and reports actual counts.

### Generator G-FANIN: high fan-in (S-003)

Purpose: exercise reverse traversal (`edgesTo`), reverse-impact queries, unresolved promotion/demotion at scale, and owner-scoped removal against a hub target, per [Storage — Store port](../storage.md#1-store-port).

Parameters:

| Parameter | Meaning |
|---|---|
| `seed` | PRNG seed; recorded with every result |
| `hubCount` | Number of hub definition nodes |
| `referrerFiles` | Number of files containing references |
| `refsPerFile` | References emitted per referrer file |
| `hubSkewPercent` | Share of all references pointing at the single hottest hub |
| `unresolvedPercent` | Share of references that intentionally name no existing definition; allocated **before** hub skew |
| `producersPerFile` | Distinct owners per file: extractor `x1` at versions `v1..v{n}`, each claiming every fact in that file |

Construction:

1. Emit `hubCount` definition files `def{ordinal}.txt`, each defining exactly one hub node `hub{ordinal}` with lookup text `hub_{ordinal}`.
2. Emit `referrerFiles` files `ref{ordinal}.txt`. Each declares exactly one source node `src{ordinal}` and `refsPerFile` reference occurrences from it. Across all `referrerFiles * refsPerFile` occurrences, allocate `unresolvedPercent` first to texts `missing_{ordinal}` that no definition file provides, then `hubSkewPercent` to `hub0`, both to the lowest occurrence ordinals; assign each remaining occurrence, in ascending occurrence ordinal, a hub drawn from `hub1..hub{hubCount-1}` by one bounded PRNG draw. `hubSkewPercent + unresolvedPercent` must not exceed 100, and a remainder greater than zero requires `hubCount >= 2`.
3. Every occurrence carries edge kind `references`, extractor identity `x1`, a distinct byte range, and no scope hint, so repeated occurrences from one source to one hub form a single canonical edge. The oracle states canonical edges and occurrence counts separately under the occurrence-visibility rule above.
4. Assign every fact in a file to all owners `(x1, v1..v{producersPerFile})`, so removing one owner must leave each canonical fact supported by the others. Removal cases always name explicit victims in this order: owner `(x1, v1)` of `ref0.txt`, then all claims of `ref0.txt`, then `def0.txt`.
5. Structural facts are limited to one node per generated file plus its `contains` edge from the root, so no oracle edge has a missing endpoint.

The generator emits an oracle containing, at minimum:

- expected total nodes, canonical edges, and reference occurrences
- expected in-degree for **every** hub, not only the hottest
- expected out-degree per referrer file
- expected unresolved-reference count per seeking text
- expected surviving in-degree for `hub0` after removing owner `(x1, v1)` of `ref0.txt` (unchanged when other owners remain) and after removing all claims of `ref0.txt`
- expected state after deleting `def0.txt`, which must remove `hub0` and demote every incoming edge back to its original unresolved occurrences

Required shape coverage: at least one case where `hub0`'s in-degree exceeds the per-file reference count by a large factor, at least one case with `hubSkewPercent = 100` and `unresolvedPercent = 0`, and at least one case whose hub in-degree exceeds the **experiment's** configured write-batch size. Batch size is an experiment parameter reported alongside the generated counts, not a property of the generator.

### Generator G-FANOUT: high fan-out

`G-FANOUT` shares `G-FANIN`'s reproducibility, ownership, and occurrence rules but is defined independently rather than by inverted parameters:

1. Emit `hubCount` definition files `def{ordinal}.txt` exactly as in `G-FANIN`.
2. Emit one file `ref0.txt` declaring source node `src0` with exactly one reference occurrence to each of `hub0..hub{hubCount-1}`, in ascending hub ordinal. No PRNG draw is required, `unresolvedPercent` is 0, and no hub is skipped.
3. Assign every fact to owners `(x1, v1..v{producersPerFile})` as in `G-FANIN`.

The oracle records expected out-degree for `src0`, expected canonical edges and occurrences, expected in-degree of exactly one for every hub, and the removal sequence: first remove owner `(x1, v1)` of `ref0.txt` and expect no canonical change, then delete `def0.txt` and expect `hub0` to disappear with its incoming edge demoted to an unresolved reference, then remove all claims of `ref0.txt` and expect every remaining hub in-degree to drop to zero.

### Generator G-REPLACE: file replacement (S-004)

Purpose: exercise per-file replacement cost and exactness — `removeClaims(owner)`, `removeByFile`, deterministic rematerialization, and unresolved promotion/demotion — per [Graph Model §3](../graph-model.md#3-provenance) and [Incremental Updates](../incremental.md). It follows the same PRNG, serialization, allocation, and symbolic-ID rules as `G-FANIN`.

Parameters:

| Parameter | Meaning |
|---|---|
| `seed` | PRNG seed |
| `fileCount` | Files in the fixture |
| `nodesPerFile` | Definition nodes generated per file |
| `edgesPerFile` | Reference occurrences generated per file |
| `unresolvedPerFile` | Additional occurrences per file that name no generated definition |
| `crossFilePercent` | Share of that file's `edgesPerFile` resolvable occurrences whose target is defined in another file |
| `producersPerFile` | Distinct owners per file: extractor `x1` at versions `v1..v{n}` |
| `sharedFactPercent` | Share of a file's facts, in construction order, claimed by **all** owners rather than only `v1` |
| `revisionSequence` | Ordered fixture-local edit labels, each mapped to a submitted change |

`nodesPerFile`, `edgesPerFile`, and `unresolvedPerFile` count generated occurrences per file, independent of ownership; the oracle reports occurrences, claims, and canonical facts separately. `crossFilePercent` applies only to `edgesPerFile`, never to `unresolvedPerFile`. `revisionSequence` labels are fixture-local and map explicitly onto the change kinds in [Incremental Updates](../incremental.md):

| Fixture label | Target | Submitted change |
|---|---|---|
| `modify-tail` | `file00000` | `Modify`: append one definition `def_{file}_{nodesPerFile}` and one reference to `def_{file}_0` |
| `replace-all` | `file00000` | `Modify` regenerating the file with the `alt_` prefix, sharing no fact with the previous version |
| `delete` | `file{fileCount-1}` | `Delete` |
| `rename` | `file00001` | `Rename` to `moved_file00001.txt` with byte-identical content |
| `resubmit` | `file00000` | resubmission of byte-identical content |
| `whitespace-tail` | `file00000` | `Modify` appending one blank line after every generated fact line |

Construction:

1. Emit `fileCount` files `file{ordinal}.txt`. Each file emits, in this order: `nodesPerFile` definition lines naming `def_{fileOrdinal}_{nodeOrdinal}`; `edgesPerFile` reference lines; then `unresolvedPerFile` reference lines naming `missing_{fileOrdinal}_{ordinal}`. Each line is one fact, so ranges are a deterministic function of the generated text. Every reference in `file{ordinal}.txt` has `from = file{ordinal}`, the file's own structural node; no separate source node is generated.
2. Within a file's `edgesPerFile` occurrences, the lowest `floor(edgesPerFile * crossFilePercent / 100)` ordinals target definitions in other files and the rest target `def_{thisFile}_{i mod nodesPerFile}`. Cross-file targets come from the flattened sequence of `(targetFileOrdinal, targetNodeOrdinal)` pairs for every file except this one, ordered ascending by file then node, selected cyclically as `j mod ((fileCount - 1) * nodesPerFile)` for cross-file occurrence index `j`. No PRNG draw is required.
3. Every occurrence uses edge kind `references` and extractor identity `x1`, so occurrences with the same source and target form one canonical edge.
4. For each file, facts are numbered in the construction order of step 1. The lowest `floor(totalFactsInFile * sharedFactPercent / 100)` facts are claimed by all owners `(x1, v1..v{producersPerFile})`; the remainder are claimed only by `(x1, v1)`. Removing `(x1, v1)` must therefore drop exactly the unshared facts and keep every shared one.
5. Each revision step is a function of the immediately preceding content of its target file. `modify-tail` appends one definition and one reference line and then reapplies step 4 to that file's full current fact list, so ownership allocation is never history-dependent. `replace-all` regenerates the target file at its current counts with every definition **and** every resolvable reference text switched to the `alt_` prefix, so the file shares no fact with its previous version and its cross-file references become unresolved. `rename` copies bytes unchanged to `moved_file00001.txt`.

Oracle:

1. Record, for the base revision and after each sequence step: canonical nodes/edges, occurrences, unresolved references, per-owner claim counts, which shared canonical facts must survive because another owner still claims them, and which incoming cross-file edges must demote to unresolved.
2. `resubmit` expects no graph change and no new graph revision. `whitespace-tail` expects stable node identities and unchanged semantic facts; ranges after each inserted blank line may shift and a new revision is permitted. A range-shift-only revision is not a failure.
3. Emit separate expectations for `removeClaims(owner)` and `removeByFile(root, path)` on `file00000`: the owned-claim difference is exactly the other owners' claims, while `removeByFile` additionally expects the file's definitions to disappear and incoming cross-file edges owned by other files to demote to unresolved references.

Required shape coverage: at least one file whose owned facts exceed the experiment's configured write-batch size, at least one file that is the target of many cross-file occurrences, and at least one file with `sharedFactPercent = 100` where removing one owner changes nothing observable in the canonical graph.

### Retained artifacts

Each generated fixture retains its generator name and version, the parameter tuple and seed, the manifest, the oracle, and the corpus band. A seed that exposes a candidate failure is retained as a named regression case with the failing shape and the observed defect, per [Storage Adapter Experiments — Shared method](storage.md#2-shared-method).

## 7. Preparation still required

Before experimentation begins:

1. choose and license-review immutable public snapshots for the four formats
2. implement or specify deterministic synthetic fixture generators
3. commit expected normalized graph/range snapshots
4. define exact pathological case limits
5. record checksums and acquisition scripts
6. label every experiment case with the corpus bands it exercises
7. implement the [deterministic graph-shape generators](#6-deterministic-graph-shape-generators) and commit their oracles
