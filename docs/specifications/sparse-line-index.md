# Specification: Sparse-checkpoint line index

```text
Status: accepted normative subsystem specification backing ADR-014
Milestone: I1 — deterministic extraction
Scope: byte-offset to line/Unicode-scalar position conversion
Primary recommendation: line starts plus sparse per-line checkpoints
Initial private stride: 128 bytes
```

## 1. Specification

Build one ephemeral line index per file content revision. The index stores the
starting byte offset of every line and adds sparse Unicode-scalar checkpoints
only where a direct byte-to-column conversion is insufficient.

```text
LineIndex
  contentLength
  lineStarts: ByteOffset[]
  specialLines: SpecialLineIndex[]

SpecialLineIndex
  lineOrdinal
  checkpoints: ScalarCheckpoint[]

ScalarCheckpoint
  byteDeltaFromLineStart
  scalarCountFromLineStart
```

`specialLines` is sparse and contains only lines with non-ASCII or invalid
input. Ordinary ASCII lines require no per-line allocation or metadata beyond
their entry in `lineStarts`.

The initial checkpoint stride is 128 bytes. The implementation places a
checkpoint at the first decoded scalar boundary at or beyond each stride
boundary, never in the middle of a UTF-8 sequence or maximal invalid run.

The stride is a private tuning constant. Changing it does not change public
coordinates, persisted graph meaning, or the extraction contract.

## 2. Construction

Construction makes one forward pass over the original file bytes:

1. record byte offset zero as the first line start;
2. detect `\n` and `\r\n` as one logical line ending while preserving their
   original byte widths;
3. track whether each line is ASCII-only;
4. decode non-ASCII bytes according to ADR-006, mapping one replacement scalar
   per maximal invalid UTF-8 run;
5. add sparse scalar checkpoints for non-ASCII or invalid-input lines; and
6. check cancellation and resource limits at bounded intervals.

Empty input contains one empty logical line. A file ending in a line terminator
also has a following empty logical line so positions at end-of-file remain
well-defined.

Construction must either return a complete index for the exact content bytes
or return a bounded failure. A partial index is never published.

## 3. Lookup

To convert a byte offset to a public position:

1. reject an offset beyond `contentLength`;
2. binary-search `lineStarts` for the containing line;
3. if the prefix from line start to offset is ASCII, compute the scalar column
   directly from the byte delta;
4. otherwise select the nearest preceding checkpoint; and
5. decode from that checkpoint to the requested offset.

The returned position follows the existing contract:

- line and column are 1-based;
- column counts Unicode scalar values;
- byte offset is 0-based;
- end offsets are exclusive;
- CRLF counts as one line ending while retaining two source bytes; and
- invalid UTF-8 contributes one replacement scalar per maximal invalid run.

Offsets inside a multi-byte scalar or invalid run are not valid evidence
boundaries. The adapter must reject them rather than round silently.

## 4. Rationale and representation comparison

| Representation | Lookup | Memory | Assessment |
| --- | ---: | ---: | --- |
| Full byte-to-position map | constant | proportional to every source byte | Fast but wasteful for ordinary source files. |
| Repeated scan from file start | proportional to preceding file bytes | minimal | Repeats work for every extracted range. |
| Line starts plus scan from line start | logarithmic line lookup plus line length | small | Adequate for normal lines but unbounded on generated/minified lines. |
| Line starts plus sparse checkpoints | logarithmic line lookup plus one stride | small and input-sensitive | Accepted baseline. |

The sparse design preserves the common ASCII fast path while bounding work on
long Unicode or minified lines. It also avoids persisting an internal
performance structure whose representation may change independently of graph
semantics.

## 5. Lifecycle and storage

The line index is keyed in memory by the file's exact content identity and is
valid only for those bytes. Extraction, direct search, lexical-hit validation,
and result assembly may share it while processing that revision.

The initial profile does not persist the line index in SQLite. Persisted facts
retain their final byte offsets and normalized public positions. On a working
tree reread, exact source evidence is verified against current bytes and a new
ephemeral index is built when conversion is required.

Implementations may use checked 32-bit offsets when the configured maximum file
size is below 4 GiB. Otherwise they use a wider checked offset type. Overflow is
a bounded diagnostic, never wrapping arithmetic.

## 6. Structural bounds

The implementation targets these structural bounds:

- at most one complete construction scan per file content revision;
- lookup work of `O(log line_count + checkpoint_stride)` bytes;
- no more than 128 bytes decoded after the nearest checkpoint under the initial
  stride, except where one scalar or maximal invalid run crosses the boundary;
- one checked line-start entry per logical line;
- checkpoints only for lines that need them;
- bounded cancellation latency during construction; and
- configured limits for file bytes, line count, line length, checkpoints, and
  requested conversions.

Wall-clock and memory thresholds remain workload measurements. A measured
threshold may tune the private stride or cache lifetime without reopening this
specification, provided the structural bounds and public coordinates remain intact.

## 7. Acceptance criteria

Implementation validation should demonstrate:

- exact agreement with the ADR-006 oracle for ASCII, Unicode, combining marks,
  tabs, CRLF, invalid UTF-8, empty input, boundaries, and long lines;
- forward and end-exclusive range conversions without off-by-one behavior;
- deterministic output for repeated construction over identical bytes;
- no repeated whole-file scans during one file-revision operation;
- bounded behavior for maximum permitted files and pathological long lines;
- cancellation never publishes a partial index; and
- equivalent public positions across reasonable private stride values.

## 8. Reopen triggers

Reconsider the representation if measured index memory is material relative to
the graph workload, construction dominates extraction, lookup cannot meet its
structural bound, or the invalid-byte/range contract cannot be implemented
without ambiguity.

## 9. Non-decisions

This specification does not change public coordinate semantics, select parser
packages, persist line indexes, or make byte offsets part of node identity. It
refines the private representation governed by
[ADR-006](../decisions/ADR-006-extraction-and-ranges.md) and accepted in [ADR-014](../decisions/ADR-014-sparse-checkpoint-line-index.md).
