# Specification: Multi-Tier Model Provider Architecture

```text
Status: accepted normative subsystem specification backing ADR-022
Milestones: I5 (Embeddings), I6 (Reranking), I7 (Host Integration), I8 (Enrichment)
Scope: model-powered capabilities (embedded, agent-powered, and standard APIs)
Primary recommendation: three-tier provider hierarchy with strict deterministic fallback
```

## 1. Scope & Architecture

Repin implements a modular **Three-Tier Model Provider Architecture** satisfying the three optional intelligence port contracts:
1. `EmbeddingModel` ([docs/intelligence.md §1](../intelligence.md#1-capability-specific-ports))
2. `Reranker` ([docs/intelligence.md §1](../intelligence.md#1-capability-specific-ports))
3. `TextModel` ([docs/intelligence.md §1](../intelligence.md#1-capability-specific-ports))

```text
                                  Repin Intelligence Ports
                    ┌────────────────────────┼────────────────────────┐
                    ▼                        ▼                        ▼
             EmbeddingModel               Reranker                TextModel
                    │                        │                        │
         ┌──────────┴────────────────────────┴────────────────────────┴──────────┐
         ▼                                   ▼                                   ▼
┌──────────────────┐               ┌──────────────────┐               ┌───────────────────┐
│ Tier 1: Embedded │               │  Tier 2: Agent   │               │Tier 3: Standard API│
│ (In-Process ONNX)│               │ (Shell Pipeline) │               │(OpenAI/Ollama/    │
│                  │               │                  │               │ Google Gemini)    │
│ • HuggingFace Hub│               │ • Stdin/Stdout   │               │ • REST JSON HTTP  │
│   on-demand fetch│               │   JSON-RPC       │               │ • Zero-secret env │
│ • Mean & Last-tok│               │ • Subagent tools │               │   key resolution  │
│ • Matryoshka MRL │               │ • Process timeout│               │ • Bounded retry   │
└──────────────────┘               └──────────────────┘               └───────────────────┘
```

---

## 2. Port Contracts & Data Shapes

### 2.1 EmbeddingModel Contract

```text
embed(request: EmbedRequest) -> Result<EmbedResponse>

EmbedRequest
  texts:            Text[]
  taskType?:        Query | Passage | Symmetric
  truncateDim?:     Count (e.g. 128, 256, 768)

EmbedResponse
  embeddings:       f32[][] (L2-normalized)
  modelIdentity:    ModelIdentity
  tokenUsage?:      Count
```

### 2.2 Reranker Contract

```text
rerank(request: RerankRequest) -> Result<RerankResponse>

RerankRequest
  query:            Text
  candidates:       RerankCandidate[]
  topN?:            Count
  deadlineMs?:      DurationMs

RerankCandidate
  id:               EntityId | Text
  content:          Text

RerankResponse
  results:          RerankHit[]
  modelIdentity:    ModelIdentity

RerankHit
  id:               EntityId | Text
  score:            f32 in [0.0, 1.0]
  rank:             Count
```

### 2.3 TextModel Contract

```text
generate(request: GenerateRequest) -> Result<GenerateResponse>

GenerateRequest
  prompt:           Text
  systemPrompt?:    Text
  maxTokens?:       Count
  temperature?:     f32

GenerateResponse
  text:             Text
  modelIdentity:    ModelIdentity
```

---

## 3. Tier 1: Embedded Local ONNX Specification

### 3.1 Model Acquisition & Cache Layout
* **Hugging Face Hub Client**: Embedded downloads use the official `hf-hub` client without requiring an API key for open weights.
* **Cache Root**: Conforms to standard user cache directories:
  * Linux: `~/.cache/repin/models/{org}/{repo}/`
  * macOS: `~/Library/Caches/repin/models/{org}/{repo}/`
  * Windows: `%LOCALAPPDATA%\repin\cache\models\{org}\{repo}\`
* **Required Files**:
  * `model.onnx` (or `onnx/model.onnx` / `model_quantized.onnx`)
  * `tokenizer.json`
  * `config.json`

### 3.2 Pooling & Normalization Rules
1. **Mean Pooling (Default / ModernBERT / GTE)**:
   $$\mathbf{v}_{\text{mean}} = \frac{\sum_{i=1}^L m_i \cdot \mathbf{h}_i}{\sum_{i=1}^L m_i}$$
   where $\mathbf{h}_i$ is the hidden state at token $i$ and $m_i \in \{0, 1\}$ is the attention mask.
2. **Last-Token Pooling (Jina v5 / EuroBERT)**:
   $$\mathbf{v}_{\text{last}} = \mathbf{h}_{\text{last\_active\_token}}$$
3. **Matryoshka Representation Learning (MRL) Truncation**:
   $$\mathbf{v}_{\text{trunc}} = \mathbf{v}[0 \dots D - 1]$$
4. **L2 Normalization**:
   $$\mathbf{v}_{\text{final}} = \frac{\mathbf{v}_{\text{trunc}}}{\|\mathbf{v}_{\text{trunc}}\|_2}$$

---

## 4. Tier 2: Agent-Powered Shell Pipeline Specification

### 4.1 Invocation Protocol
* **Execution**: Command specified in `agent_cmd` (e.g. `claude-agent-rerank --json`).
* **Communication**: Structured JSON over standard input/output.
* **Timeout**: Subprocess execution is strictly deadline-bounded (default: 10,000ms).

### 4.2 Stdin/Stdout Schema

**Input (JSON sent via stdin):**
```json
{
  "jsonrpc": "2.0",
  "method": "repin/rerank",
  "params": {
    "query": "session eviction timer",
    "candidates": [
      { "id": "fn:reap_idle_leases", "content": "pub fn reap_idle_leases(&mut self) { ... }" },
      { "id": "fn:connect_client", "content": "pub fn connect_client(...) { ... }" }
    ]
  }
}
```

**Output (JSON received via stdout):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "ranked": [
      { "id": "fn:reap_idle_leases", "score": 0.94 },
      { "id": "fn:connect_client", "score": 0.12 }
    ]
  }
}
```

---

## 5. Tier 3: Standard Remote APIs Specification

### 5.1 Protocol Mapping

| Provider Type | Embedding Endpoint | Reranking Endpoint | Text Generation Endpoint |
| :--- | :--- | :--- | :--- |
| **`openai`** | `POST /v1/embeddings` | `POST /v1/rerank` (or Jina/TEI/Cohere compat) | `POST /v1/chat/completions` |
| **`ollama`** | `POST /api/embeddings` | *(via local cross-encoder model)* | `POST /api/generate` |
| **`google`** | `POST /v1beta/models/{model}:embedContent` | *(via Gemini cross-attention prompt)* | `POST /v1beta/models/{model}:generateContent` |

### 5.2 Zero-Secret Credential Resolution
Configuration files MUST NOT contain raw API tokens. Tokens are referenced strictly via environment variable names:
1. `api_key_env = "OPENAI_API_KEY"`
2. `api_key_env = "GEMINI_API_KEY"`

If the environment variable is missing, Repin logs an honest degradation diagnostic and gracefully falls back to deterministic graph results without crashing.

---

## 6. Configuration Schema & Scope Boundaries

To prevent leaking API secrets or private endpoint URLs in version-controlled repositories:
1. **`[intelligence.providers.<name>]`** and **`api_key_env`** are **strictly global** (`~/.config/repin/config.toml`).
2. **Project configurations** (`<root>/.repin/config.toml` or `<root>/config.toml`) are rejected if they contain `[intelligence.providers]` or `api_key_env`.

### 6.1 Global Configuration (`~/.config/repin/config.toml`)
```toml
# --- Shared Provider Profiles (Global Only) ---
[intelligence.providers.openai]
endpoint = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"

[intelligence.providers.ollama]
endpoint = "http://localhost:11434"

[intelligence.providers.google]
endpoint = "https://generativelanguage.googleapis.com"
api_key_env = "GEMINI_API_KEY"
```

### 6.2 Project Configuration (`<root>/.repin/config.toml`)
```toml
# --- Feature Capabilities (Safe for Git / Team sharing) ---
[intelligence.embedding]
provider = "none" # "none" | "embedded" | "agent" | "openai" | "ollama" | "google"
model = "Alibaba-NLP/gte-modernbert-base"
dimension = 256
auto_download = true

[intelligence.rerank]
provider = "none" # "none" | "embedded" | "agent" | "openai" | "ollama" | "google"
model = "Alibaba-NLP/gte-reranker-modernbert-base"
top_n = 50
deadline_ms = 100
agent_cmd = "my-agent-rerank --json"

[intelligence.enrichment]
provider = "none" # "none" | "google" | "openai" | "ollama" | "agent"
model = "gemini-2.5-flash"
```

### 6.3 Provider Resolution Algorithm
```text
For a given capability (e.g. embedding) with provider P:
1. If P == "none": capability is disabled.
2. If P == "embedded": load local ONNX model from cache/disk with zero network.
3. If P == "agent": execute `agent_cmd` subprocess via stdin/stdout JSON-RPC.
4. If P is a remote API provider ("openai", "ollama", "google", or custom alias):
   a. Reject if P or credentials were defined in project-level config.
   b. Look up definition in user global config `[intelligence.providers.P]`.
   c. If absent, fall back to built-in standard defaults for known providers.
   d. Resolve API token from `std::env::var(api_key_env)`.
   e. If API token is missing or network call fails, log diagnostic and fall back to deterministic retrieval.
```

---

## 7. Invariants & Conformance Criteria

1. **Deterministic Preservation**: A full repository index or update MUST complete deterministically regardless of whether model capabilities succeed, lag, or fail.
2. **Score Range**: All semantic and rerank scores MUST be normalized to the range $[0.0, 1.0]$.
3. **Rank Explanation**: In fused hybrid retrieval, each result explains its final ranking with explicit component breakdowns (`lexicalScore`, `symbolScore`, `vectorScore`, `rerankScore`).
4. **Air-Gapped Operation**: When `auto_download = false` and `provider = "embedded"`, Repin runs 100% offline with zero outbound network attempts.
