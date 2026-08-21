# Code and Architecture

This section is the authoritative documentation for Repin's implementation: architecture, contracts, data models, algorithms, verification rules, decisions, and technology choices.

Start with [Introduction](introduction.md), then read [Architecture and Layers](architecture.md). The remaining documents use that vocabulary and define the behavior that implementations and tests must preserve.

## Core design

- [Architecture and Layers](architecture.md)
- [Safety and Data Handling](safety.md)
- [Results and Evidence](results.md)
- [Graph Model](graph-model.md)
- [Extraction and Language Packs](extraction.md)
- [Incremental Updates](incremental.md)
- [Storage](storage.md)

## Interfaces and subsystems

- [Retrieval](retrieval.md)
- [Public API](api.md)
- [Runtime and IPC](runtime.md)
- [Host Integration](host-integration.md)
- [Optional Intelligence](intelligence.md)
- [Embedded RAG Proof](embedded-rag.md)
- [Subsystem Specifications](specifications/index.md)

## Delivery and rationale

- [Conformance](conformance.md)
- [Technology Selections](technology-candidates.md)
- [Roadmap](roadmap.md)
- [Benchmarks](benchmarks/library-extraction-baseline.md)
- [Architectural Decision Records](decisions/index.md)
- [Concluded Research](research/index.md)

Usage instructions belong in the [Usage Guide](../usage/index.md). They explain how to operate the current CLI without making the user read implementation contracts first.
