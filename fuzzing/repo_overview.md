# How Dynamo Works

## The problem Dynamo solves

When you send a chat message to an LLM API, something has to accept your HTTP request, figure out which GPU should run the inference, run the model, and stream tokens back. If you have one GPU, that's trivial. If you have dozens of GPUs across multiple machines -- each with limited memory, each running multiple requests concurrently -- you need a system that routes requests intelligently, manages GPU memory, and scales capacity up and down.

Dynamo is that system. It sits between the HTTP API and the actual inference engines (vLLM, SGLang, TensorRT-LLM), and its main value-add over a simple load balancer is **KV-cache-aware routing**: it tracks which intermediate computations are cached on which GPU, and routes new requests to GPUs that can reuse those cached results.

---

## Background: how LLM inference actually works

To understand the code, you need to understand two phases of LLM inference:

### Prefill

When you send a prompt to an LLM, the model processes your entire input at once in a single forward pass. This produces the **KV cache** -- a set of intermediate tensors (key/value pairs from each attention layer) that represent the model's "understanding" of your input. Prefill is compute-intensive: it does matrix multiplications over the full prompt length.

The KV cache is physically a block of GPU memory per attention layer. For a 70B parameter model with a 4096-token prompt, the KV cache might be several gigabytes. It's structured as `[num_layers × 2 × num_heads × tokens × head_dim]` tensors.

### Decode

After prefill, the model generates output tokens one at a time. Each new token requires reading the entire KV cache (to attend to all previous tokens) but only computing attention for one new position. So decode is memory-bandwidth-bound, not compute-bound.

Crucially: the KV cache from prefill is **reusable**. If two requests share the same system prompt (which is extremely common -- think "You are a helpful assistant..."), the KV cache for those shared tokens is identical. If a GPU already has that cache in memory, it can skip recomputing it entirely.

### Why this matters for routing

A naive load balancer would spread requests evenly across GPUs. But if GPU-A already has the KV cache for "You are a helpful assistant" and GPU-B doesn't, sending a new request with that same prefix to GPU-A saves all that prefill computation. Dynamo's router tracks which KV cache blocks exist on which GPU and routes accordingly.

### Disaggregated serving

Dynamo can also split prefill and decode onto separate machines. A "prefill worker" computes the KV cache, then transfers it over the network (GPU-to-GPU via NVIDIA's NIXL library) to a "decode worker" that generates tokens. This lets you use different hardware for each phase, and scale them independently.

---

## Walking through a request

Let's trace what happens when a user sends `POST /v1/chat/completions` to Dynamo. We'll follow the code path through the real files.

### Step 1: The HTTP request arrives

The entry point is Python. `components/src/dynamo/frontend/__main__.py` calls `main()` in `components/src/dynamo/frontend/main.py`, which calls `uvloop.run(async_main())`. The `async_main()` function creates a `DistributedRuntime` (the Rust runtime exposed to Python via PyO3 bindings), builds an engine configuration, and then calls:

```python
await run_input(runtime, "http", engine)
```

This crosses the Python-Rust boundary. `run_input` is defined in `lib/bindings/python/rust/llm/entrypoint.rs` and calls into `lib/llm/src/entrypoint/input.rs`. For HTTP input, that delegates to `lib/llm/src/entrypoint/input/http.rs`, which builds an `HttpService` and calls `axum::serve()` (in `lib/llm/src/http/service/service_v2.rs`) to start the HTTP server.

The Axum router registers OpenAI-compatible routes. The chat completions handler lives in `lib/llm/src/http/service/openai.rs` -- the function `handler_chat_completions()` receives the request, and ultimately calls `engine.generate(request)`.

### Step 2: Preprocessing

Before the request reaches a GPU worker, it goes through preprocessing in `lib/llm/src/preprocessor.rs`. The `OpenAIPreprocessor` does two things:

1. **Template application**: Takes the chat messages and renders them through a Jinja2 chat template (the same templates you see on HuggingFace model cards). This produces the raw text prompt.
2. **Tokenization**: Converts the text into token IDs using the model's tokenizer (HuggingFace `tokenizers` or TikToken).

The result is a `PreprocessedRequest` (defined in `lib/llm/src/protocols/common/preprocessor.rs`) containing the `token_ids` plus routing hints (like preferred worker IDs, LoRA adapter name, etc.).

Note: block hash computation does **not** happen in the preprocessor. It happens later, in the KV router layer (step 3), when the router calls `compute_block_hash_for_seq()` on the token IDs. This function lives in `lib/kv-router/src/protocols.rs` (re-exported via `lib/llm/src/kv_router.rs`) and splits the tokens into fixed-size blocks, hashing each one. The hashes are what let the router answer: "does GPU X have the KV cache for the first N blocks of this request?"

### Step 3: Routing

The preprocessed request reaches the router. There are two modes:

**Simple mode** (round-robin or random): The `PushRouter` in `lib/runtime/src/pipeline/network/egress/push_router.rs` picks a worker using basic load balancing. It watches the service discovery system for available workers and cycles through them.

**KV-aware mode**: The `KvPushRouter` in `lib/llm/src/kv_router/push_router.rs` does something smarter. Its `select_worker()` method extracts the token IDs from the `PreprocessedRequest`, then calls `find_best_match()` on the `KvRouter`. Here's what happens inside:

1. The router hashes the request's tokens into block-sized chunks via `compute_block_hash_for_seq()`.
2. It walks the radix tree (in `lib/kv-router/src/radix_tree.rs`) -- a tree where each node represents a token block, and each node tracks which workers have that block cached. Walking the tree with the request's block sequence tells you: "Worker A has the first 5 blocks cached, Worker B has the first 12 blocks cached, Worker C has none."
3. Compute a cost for each worker. In the scheduler (`lib/llm/src/kv_router/scheduler.rs`), the formula is: `logit = overlap_weight × potential_prefill_blocks + decode_blocks`. `potential_prefill_blocks` is the number of blocks the worker would need to compute (total minus cached). `decode_blocks` is an estimate of the decode load the worker would carry if this request were assigned to it. Lower cost = better choice.
4. Pick the worker with the lowest cost.

**Disaggregated mode**: If prefill and decode are split, the `PrefillRouter` in `lib/llm/src/kv_router/prefill_router.rs` orchestrates a two-step process:
1. Route to a prefill worker, which computes the KV cache and returns metadata about where it's stored.
2. Route to a decode worker, passing along the KV transfer metadata so the decode worker can pull the KV cache from the prefill worker's GPU.

### Step 4: Transport to the worker

The router sends the request to the chosen worker. The transport layer (`lib/runtime/src/pipeline/network/`) supports three protocols, configured via the `DYN_REQUEST_PLANE` environment variable:

- **TCP** (default): A custom binary protocol. The wire format in `lib/runtime/src/pipeline/network/codec/zero_copy_decoder.rs` is: `[path_len: u16][path][headers_len: u16][headers as JSON][payload_len: u32][payload]`. Zero-copy decoding via the `bytes` crate.
- **HTTP/2**: Standard HTTP, easier to debug.
- **NATS**: Message broker-based, pub/sub patterns.

The `AddressedPushRouter` in `lib/runtime/src/pipeline/network/egress/addressed_router.rs` handles the actual send. It uses a `RequestPlaneClient` trait that abstracts over the transport -- the same `send_request()` call works regardless of whether the underlying transport is TCP, HTTP/2, or NATS.

### Step 5: Worker runs inference

On the worker side, a Python process is running one of the inference backends. For vLLM, this is `components/src/dynamo/vllm/main.py`. The worker registers a `generate` endpoint with the discovery system, and handler classes (`PrefillWorkerHandler`, `DecodeWorkerHandler` in `components/src/dynamo/vllm/handlers.py`) process incoming requests.

The worker calls into vLLM / SGLang / TRT-LLM to do the actual GPU inference. The details of this are outside Dynamo's scope -- Dynamo just orchestrates around these engines.

### Step 6: Streaming the response back

The worker generates tokens one at a time and streams them back. Responses flow back through the pipeline as `ManyOut<Annotated<LLMEngineOutput>>` -- a stream of annotated output chunks. The HTTP layer converts these into Server-Sent Events (SSE) and streams them to the client. The SSE handling is in `lib/llm/src/http/service/openai.rs`, with client disconnect detection so we stop inference if the client hangs up.

---

## How services find each other

When a worker starts, it needs to announce itself. When the router starts, it needs to discover workers. This is the service discovery system in `lib/runtime/src/discovery/`.

### Registration

A worker creates a `Component` and an `Endpoint`, then registers with the discovery backend. In etcd mode (`lib/runtime/src/transports/etcd.rs`), this writes a key like:

```
v1/instances/{namespace}/{component}/{endpoint}/{instance_id_hex}
```

The value is a JSON `Instance` struct with the worker's namespace, component name, endpoint name, instance ID, and transport type. Each key has an etcd lease (default 10-second TTL) with a background keep-alive. If the worker dies, the lease expires, etcd deletes the key, and other services see the worker disappear.

In Kubernetes mode, workers create `DynamoWorkerMetadata` custom resources and use EndpointSlices.

### Discovery

Consumers (like the router) watch key prefixes. `kv_get_and_watch_prefix()` first fetches existing keys, then establishes a watch stream for new Put/Delete events. When a new worker appears or an existing one dies, the router's `PushRouter` updates its list of available targets.

### KV cache events

When a worker stores or evicts KV cache blocks, it publishes events via `lib/llm/src/kv_router/publisher.rs`. The event transport depends on configuration: when the router runs a local indexer (co-located with the worker), events can flow over ZMQ for lower latency. When the indexer is remote, events flow over NATS JetStream for durability. The event contains:
- The block's `LocalBlockHash` (content hash of the tokens in the block) and `ExternalSequenceBlockHash` (cumulative sequence hash)
- The worker ID and data-parallel rank
- Parent block hash (for building the tree structure)
- Token IDs and block size (so the indexer can recompute hashes if needed)

The router subscribes to these events. The `ThreadPoolIndexer` in `lib/kv-router/src/indexer.rs` receives them and updates the radix tree: `Stored` events add workers to tree nodes (creating new nodes as needed); `Removed` events remove the worker from nodes (pruning empty subtrees). This keeps the router's view of "which GPU has which cached blocks" up to date.

---

## The KV block management system

The KV cache on each worker is managed by a block system spanning three crates.

### Token hashing (`lib/tokens/`)

Tokens get chunked into fixed-size blocks. Each block is hashed to produce identifiers that enable cache lookup. The key data structure is `TokenBlock` (in `lib/tokens/src/lib.rs`):

```rust
struct TokenBlock {
    tokens: Tokens,                              // newtype over Vec<u32>
    salt_hash: SaltHash,                         // u64 -- seed for hashing
    block_hash: BlockHash,                       // u64 -- hash of just this block's tokens
    sequence_hash: SequenceHash,                 // u64 -- cumulative hash of entire prefix
    parent_sequence_hash: Option<SequenceHash>,  // u64 -- parent block's sequence hash
    positional_sequence_hash: PositionalSequenceHash,  // u128 -- content + position
    positional_lineage_hash: PositionalLineageHash,    // u128 -- content + position + parent chain
}
```

A `TokenBlockSequence` accumulates tokens and auto-commits full blocks when they reach `block_size` (configurable -- the codebase uses values like 4 in tests and larger powers of 2 in production). The `PositionalRadixTree` (in `lib/tokens/src/radix.rs`) is a sparse two-level `DashMap<u64, DashMap<K, V>>` where the outer map is keyed by block position and the inner map by hash. This provides concurrent lookup by position and hash.

### Block lifecycle (`lib/kvbm-logical/`)

Each KV cache block on a GPU goes through a lifecycle managed by a type-state pattern (compile-time state enforcement via Rust's type system):

```
ResetPool (free blocks)
    │
    ▼ allocate
MutableBlock ── stage() ──▶ CompleteBlock ── register() ──▶ ImmutableBlock
    │                                                           │
    │                                                    reference dropped
    │                                                           │
    │                                                           ▼
    │                                                      WeakBlock
    │                                                     (InactivePool)
    │                                                           │
    └──────────────────── reset ◀───────────── evict ───────────┘
```

Three pools manage blocks:
- **ResetPool**: Free blocks available for allocation. Backed by a `VecDeque` (FIFO) inside a `DequeBlockAllocator`.
- **ActivePool**: Blocks currently in use by active requests. Wraps a `BlockRegistry` backed by a `PositionalRadixTree` for prefix-aware lookups.
- **InactivePool**: Cached blocks not currently in use but potentially reusable. Uses a pluggable `InactivePoolBackend` trait with four implementations:
  - `HashMapBackend`: Simple hash map with configurable reuse policy.
  - `LruBackend`: Single LRU cache.
  - `MultiLruBackend`: Four frequency tiers (cold/warm/hot/very hot). Uses a TinyLFU Count-Min Sketch for frequency tracking. Evicts from the coldest tier first.
  - `LineageBackend`: Eviction aware of parent-child block relationships.

The `BlockManager` in `lib/kvbm-logical/src/manager/` orchestrates all three pools.

### GPU transfers (`lib/kvbm-kernels/`)

When blocks need to move between storage tiers (GPU ↔ CPU ↔ disk) or between machines, `kvbm-kernels` handles the physical copies. Key operations:
- `vectorized_copy`: Batched copy of `(src, dst)` pointer pairs with runtime alignment detection (16/8/4/1-byte loads).
- `memcpy_batch`: Uses CUDA's `cudaMemcpyBatchAsync` (CUDA 12.9+) with fallback to individual copies.
- Permute kernels: Convert between "block stack" layout (vLLM's format) and "universal" layout (Dynamo's storage format).

These are actual CUDA C++ kernels compiled via `build.rs`. When no GPU is available, CPU stubs are compiled instead (they abort if called).

### Memory abstraction (`lib/memory/`)

The `Buffer` type wraps `Arc<dyn MemoryDescriptor>` to provide type-erased memory. The `StorageKind` enum classifies memory into four variants:

| StorageKind | Backing type | Use |
|-------------|-------------|-----|
| `Device(device_id)` | `DeviceStorage` -- CUDA GPU memory | Active inference |
| `Pinned` | `PinnedStorage` -- CUDA pinned host memory | Staging for GPU transfers |
| `Disk(id)` | `DiskStorage` -- mmap'd files (Linux only) | Large cache persistence |
| `System` | System memory (malloc) | General host-side use |

There's also `ExternalDeviceMemory`, which doesn't own memory -- it wraps a raw pointer to GPU memory allocated by an external framework (e.g., vLLM's KV cache tensors). Its purpose is to register that memory with NIXL for RDMA transfers without copying it.

In the KVBM design docs, these tiers are sometimes referred to as G1 (device), G2 (pinned host), G3 (disk), G4 (remote/external), but those labels are conceptual -- the code uses `StorageKind` and zero-sized marker types like `struct G1;`, `struct G2;` as generic parameters on `BlockManager<T>` to prevent mixing blocks from different tiers at compile time.

---

## The radix tree: how cache lookup works

The radix tree in `lib/kv-router/src/radix_tree.rs` is the core data structure for KV-aware routing. Here's what it looks like conceptually:

```
              [root]
             /      \
       [block_0a]   [block_0b]      ← first block of different prompts
       {W1, W2}     {W3}            ← workers that have this block cached
          |            |
       [block_1a]   [block_1b]
       {W1}         {W3}
          |
       [block_2a]
       {W1}
```

Each node is a `RadixBlock`:
```rust
struct RadixBlock {
    children: FxHashMap<LocalBlockHash, SharedRadixBlock>,
    workers: FxHashSet<WorkerWithDpRank>,
    block_hash: Option<ExternalSequenceBlockHash>,
    recent_uses: VecDeque<Instant>,
}
```

The tree also has a flat lookup table: `WorkerWithDpRank → (ExternalSequenceBlockHash → SharedRadixBlock)` for O(1) access when processing events (so you don't have to walk the tree to find where a worker's blocks are).

**Lookup** (`find_matches`): Given a sequence of `LocalBlockHash`es from a request, walk the tree from root. At each level, check which workers have matching children. Track how deep each worker matches. The result is an `OverlapScores` map: `WorkerWithDpRank → matched_depth` (plus tree size per worker). A worker that matches 12 out of 15 blocks can skip 12 blocks of prefill computation.

**Updates**: When a worker stores or evicts blocks, events arrive via the event plane (NATS or ZMQ depending on configuration). `Stored` events add the worker to each block's `workers` set (creating nodes as needed). `Removed` events remove the worker; if no workers remain at a node, its subtree is pruned.

The `ConcurrentRadixTree` in `lib/kv-router/src/concurrent_radix_tree.rs` wraps each node in `Arc<RwLock<>>` for thread-safe concurrent access.

---

## The Python-Rust boundary

The Rust crates are exposed to Python via two maturin-built packages:

**`ai-dynamo-runtime`** (`lib/bindings/python/`): Exposes the `DistributedRuntime` class to Python. Key methods:
- `endpoint(path)`: Create an endpoint for serving or calling.
- `serve_endpoint(generator)`: Start serving requests with a Python async generator.
- `client(router_mode)`: Get a client that can call endpoints with round-robin, random, or KV-aware routing.

**`kvbm`** (`lib/bindings/kvbm/`): Exposes KV block manager operations for GPU memory management from Python workers.

The Python services (`components/src/dynamo/`) use these bindings:
- **Frontend** calls `run_input()` which starts the Axum HTTP server in Rust.
- **Router** creates a `KvRouter` Python object (wrapping Rust's `KvPushRouter`) and calls `generate()` on it.
- **Workers** call `serve_endpoint()` with a Python async generator that wraps vLLM/SGLang/TRT-LLM inference calls.

---

## The Planner: scaling up and down

The planner (`components/src/dynamo/planner/`) monitors system health and adjusts worker counts. It scrapes Prometheus metrics:
- `time_to_first_token_seconds` (TTFT): How long until the first token. High TTFT means not enough prefill capacity.
- `inter_token_latency_seconds` (ITL): Time between generated tokens. High ITL means not enough decode capacity.
- `active_prefill_tokens` and `active_decode_blocks`: Current load on workers.

Based on thresholds, the planner scales workers up or down. In disaggregated mode (`components/src/dynamo/planner/utils/disagg_planner.py`), prefill and decode workers scale independently.

---

## Output parsing (`lib/parsers/`)

When an LLM generates structured output (tool calls, reasoning traces), Dynamo needs to parse it. The parsers crate handles two categories:

- **Tool calling parsers**: Extract function calls from LLM output in various formats -- XML (glm47, kimi_k2), JSON (base, deepseek_v3, deepseek_v3_1), pythonic, DSML, harmony. Each parser takes a raw string and returns structured tool call objects.
- **Reasoning parsers**: Extract chain-of-thought reasoning from models that produce it -- base, gpt_oss, granite, minimax_append_think, deepseek_r1, qwen3, nemotron_deci, kimi, kimi_k25, step3, mistral. These separate the "thinking" from the final answer.

All parsers take untrusted string input from the LLM and try to parse it. This is the best fuzzing target in the repo -- there's no validation before these parsers see data.

---

## Infrastructure

Two external services are required:

- **NATS** (port 4222): Message broker. Used for the KV event plane (when the indexer is remote), optionally for the request plane, and for service discovery KV storage (via JetStream). `docker-compose.yml` runs it.
- **etcd** (port 2379): Key-value store for service discovery. Workers register; routers watch. Also defined in `docker-compose.yml`. Can be replaced by Kubernetes-native discovery.

Deployment uses multi-stage Dockerfiles in `container/` and a Go-based Kubernetes operator in `deploy/operator/` that manages `DynamoComponentDeployment` CRDs.

---

## Summary of code organization

| Path | What lives there |
|------|-----------------|
| `lib/llm/` | The main integration crate. HTTP/gRPC services, preprocessing, routing orchestration, model management. |
| `lib/runtime/` | Distributed systems backbone. Component registration, service discovery, transports (TCP/HTTP/NATS), pipeline abstractions. |
| `lib/tokens/` | Token block representation and hashing. `TokenBlock`, `TokenBlockSequence`, hash types, `PositionalRadixTree`. |
| `lib/kv-router/` | Radix tree for KV cache lookup. `RadixTree`, `ConcurrentRadixTree`, indexer that processes KV events. |
| `lib/kvbm-logical/` | Block lifecycle management. Type-state blocks, three-tier pool system, eviction backends. |
| `lib/kvbm-kernels/` | CUDA kernels for GPU memory copies and layout permutation. |
| `lib/memory/` | Type-erased memory abstraction across GPU/CPU/disk/remote tiers. |
| `lib/parsers/` | LLM output parsers for tool calling and reasoning extraction. |
| `lib/async-openai/` | Forked `async-openai` with OpenAI API type definitions. |
| `lib/config/` | Small utility crate for config/env parsing. |
| `lib/mocker/` | Mock scheduler and KV manager for testing without GPUs. |
| `lib/bench/` | HTTP benchmarking tool. |
| `lib/bindings/python/` | PyO3 bindings exposing Rust runtime to Python. |
| `lib/bindings/kvbm/` | PyO3 bindings for KV block manager. |
| `components/src/dynamo/` | Python services: frontend, router, planner, vllm/sglang/trtllm workers. |
| `deploy/` | Kubernetes operator (Go), Helm charts, inference gateway. |
| `container/` | Dockerfiles for building container images. |
| `docs/` | Design documents (KVBM architecture, router design, disaggregated serving). |
