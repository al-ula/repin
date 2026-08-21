# ADR-022: Multi-Tier Model Provider Architecture (Embedded, Agent-Powered, and Standard APIs)

```text
Status: accepted architecture and provider contract decision
Milestones: I5 (Embeddings), I6 (Reranking), I7 (Host Integration), I8 (Enrichment)
Deciders: Repin Architecture & Engineering
Backs: docs/intelligence.md, docs/specifications/vector-search-rust-friendly.md
```

## 1. Context and Problem Statement

Repin requires model-backed intelligence capabilities (semantic vector retrieval, cross-encoder reranking, and graph enrichment) that remain strictly optional without impeding deterministic indexing and retrieval.

Developers and agent environments operate under diverse operational constraints:
1. **Air-gapped / Local-First**: Zero external network egress, embedded local CPU inference, and lightweight model management.
2. **Autonomous Agent Hosts**: Agents invoking CLI tools or subagent shell pipes without needing remote API keys.
3. **Enterprise & Cloud Infrastructure**: Standard cloud endpoints (OpenAI-compatible, Ollama on localhost/LAN, and Google Gemini API).

A unified, modular provider architecture is required to satisfy the `EmbeddingModel`, `Reranker`, and `TextModel` port contracts across these three distinct operational tiers.

## 2. Decision

Repin adopts a **Three-Tier Provider Architecture** for all model-backed intelligence ports:

```text
                      ┌────────────────────────────────────────┐
                      │    Repin Intelligence Ports            │
                      │  [EmbeddingModel | Reranker | TextModel]│
                      └──────────────────┬─────────────────────┘
                                         │
         ┌───────────────────────────────┼───────────────────────────────┐
         ▼                               ▼                               ▼
 ┌─────────────────┐           ┌─────────────────┐             ┌───────────────────┐
 │ Tier 1: Embedded│           │ Tier 2: Agent   │             │ Tier 3: Remote API│
 │ (Local ONNX)    │           │ (Shell Pipeline)│             │ (OpenAI/Ollama/   │
 │                 │           │                 │             │  Google Gemini)   │
 │ • Pure Rust/ONNX│           │ • Stdin/Stdout  │             │ • HTTP JSON REST  │
 │ • HuggingFace   │           │ • Shell callback│             │ • Env-based auth  │
 │   Hub download  │           │ • Agent tool-use│             │ • Stream & batch  │
 │ • Matryoshka MRL│           │   delegation    │             │ • Retries/backoff │
 └─────────────────┘           └─────────────────┘             └───────────────────┘
```

### Tier 1 — Embedded Local ONNX Provider
- **Runtime**: In-process ONNX runtime (`ort`) or `fastembed` execution over CPU/GPU execution providers.
- **Model Acquisition**: On-demand download from Hugging Face Hub (`hf-hub` crate) into standard user cache (`~/.cache/repin/models/`).
- **Default Targets**:
  - *Embeddings*: `Alibaba-NLP/gte-modernbert-base` (Apache-2.0, 149M params, 8k context, Matryoshka 256-dim) or `jinaai/jina-embeddings-v5-text-nano` (CC-BY-NC-4.0).
  - *Reranking*: `Alibaba-NLP/gte-reranker-modernbert-base` (Apache-2.0, 149M params, 8k context) or `mixedbread-ai/mxbai-rerank-base-v1`.
- **Invariants**: Single-file model loading, standard mean/last-token pooling, deterministic score normalization in $[0.0, 1.0]$.

### Tier 2 — Agent-Powered Provider
- **Runtime**: Subprocess shell execution (`agent_cmd`) communicating via structured JSON-RPC over stdin/stdout.
- **Use Case**: Enables IDE agents (Antigravity, Claude Code, Cursor) and terminal pipelines to serve as the intelligence engine without storing cloud API keys in Repin.
- **Invariants**: Strict execution deadlines (default 10s), stderr capturing, and non-blocking fallback to deterministic rankings upon exit code failures.

### Tier 3 — Standard HTTP APIs
Repin implements native HTTP client adapters for the three standard API protocols:
1. **OpenAI-Compatible (`openai`)**:
   - Endpoints: `/v1/embeddings`, `/v1/chat/completions`, `/v1/rerank` (supporting vLLM, LocalAI, TEI, LiteLLM, and OpenAI).
2. **Ollama Native (`ollama`)**:
   - Endpoints: `/api/embeddings`, `/api/generate` for zero-configuration localhost Ollama instances.
3. **Google Gemini (`google`)**:
   - Endpoints: `v1beta/models/{model}:embedContent` and `v1beta/models/{model}:generateContent`.

## 3. Configuration Contract & Scope Boundaries

To prevent leaking credentials or binding teams to specific accounts in version-controlled repositories, **API provider profiles (`[intelligence.providers]`) and API key environment references (`api_key_env`) are strictly restricted to the User Global Configuration (`~/.config/repin/config.toml`)**.

### A. User Global Configuration (`~/.config/repin/config.toml`)
Hosts all private provider definitions, endpoints, and credentials:

```toml
# Defined once globally per developer machine
[intelligence.providers.openai]
endpoint = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"

[intelligence.providers.ollama]
endpoint = "http://localhost:11434"

[intelligence.providers.google]
endpoint = "https://generativelanguage.googleapis.com"
api_key_env = "GEMINI_API_KEY"
```

### B. Project Configuration (`<root>/.repin/config.toml`)
Project configuration only selects the provider name and model, and is **strictly prohibited** from declaring `[intelligence.providers]` or `api_key_env`:

```toml
# Safe to commit to git: zero API keys or private endpoints
[intelligence.embedding]
provider = "none" # "none" | "embedded" | "agent" | "openai" | "ollama" | "google"
model = "Alibaba-NLP/gte-modernbert-base"
dimension = 256
auto_download = true

[intelligence.rerank]
provider = "none"
model = "Alibaba-NLP/gte-reranker-modernbert-base"
top_n = 50
deadline_ms = 100

[intelligence.enrichment]
provider = "none"
model = "gemini-2.5-flash"
```

## 4. Invariants and Safety Guarantees

1. **Deterministic Asynchrony**: Background semantic embedding and enrichment queues MUST NEVER block deterministic index commits or queries ([ADR-002](ADR-002-synchronous-core.md), [docs/intelligence.md](../intelligence.md#4-asynchrony)).
2. **Fail-Safe Fallback**: Any model failure, network timeout, rate limit, or exit-code error yields status `ok` with deterministic graph ranking, never failing the search query.
3. **Global-Only API Secrets**: Project configurations (`.repin/config.toml`, `config.toml`) are rejected if they attempt to define `[intelligence.providers]` or `api_key_env`. All API endpoints and credentials resolve strictly from the user's private global configuration (`~/.config/repin/config.toml`).
4. **Offline & Disabled Default**: All model capabilities default to `enabled = false` and `provider = "none"`. Zero downloads or network calls occur without explicit user opt-in.
