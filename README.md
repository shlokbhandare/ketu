# Ketu

Ketu sits between users and multiple local AI backends, deciding which model should handle each incoming request. It tracks the number of requests per minute and the latency of every request, giving visibility into how the system is performing under load.

## How This Was Built

I (Shlok) am the Architect, assisted by Claude as a mentor, designing every component, data flow, and technical decision. AI (Cursor/Continue.dev) is the Builder, generating the Rust syntax. Every architectural decision in this project can be explained and defended by me without relying on the AI that helped write it or the AI that helped plan it.

## Architecture

```text
┌─────────┐
│  Client │
└────┬────┘
     │ POST /route
     ▼
┌─────────────────────┐
│    Rate Limiter     │  checks IP + request count (last 60s)
└──────────┬──────────┘
           │
           ├── over limit ──────────► 429 Too Many Requests ──► Client
           │
           │ under limit
           ▼
┌─────────────────────┐
│     BackendPool     │◄─────────────────────────┐  round-robin + weighted
└──────────┬──────────┘                          │  selection
           │                                     │
           ▼                                     │
┌─────────────────────┐                          │
│      ollama.rs      │  forwards request        │
└──────────┬──────────┘                          │
           │                                     │
           ▼                                     │
┌─────────────────────┐                          │
│   Ollama Backend    │  generates response      │
└──────────┬──────────┘                          │
           │                                     │
           ├── fails / timeout (max 2 attempts) ─┘
           │
           │ success
           ▼
┌─────────────────────┐
│      Response       │──► Client
└─────────────────────┘
```
## Multi-Node Cluster Architecture (Phase 2)

```text
┌──────────────────────────────┐                ┌──────────────────────────────┐
│           Router A           │  Heartbeats    │           Router B           │
│     (Leader / Port 3000)     │◄──────────────►│    (Follower / Port 3001)    │
│                              │   Rate Limits  │                              │
└──────────────┬───────────────┘                └──────────────┬───────────────┘
               │                                               │
    ┌──────────┴──────────┐                         ┌──────────┴──────────┐
    │                     │                         │                     │
    ▼                     ▼                         ▼                     ▼
┌─────────┐           ┌─────────┐               ┌─────────┐           ┌─────────┐
│ Ollama1 │           │ Ollama2 │               │ Ollama1 │           │ Ollama2 │
│  :11434 │           │  :11435 │               │  :11434 │           │  :11435 │
└─────────┘           └─────────┘               └─────────┘           └─────────┘
```

## Distributed Architecture & Design Decisions

In Phase 2, Ketu evolved from a single-instance gateway into a self-healing distributed cluster. Running multiple router instances introduces challenges around coordination, split-brain scenarios, and infinite forwarding loops. Below is how Ketu addresses these challenges:

### 1. Peer Discovery & Heartbeat Monitoring
- **Mechanism:** Nodes accept a `--peer` CLI flag on startup to discover and pair with an existing cluster member.
- **Dead Peer Detection:** Nodes exchange background HTTP heartbeats every 10 seconds. If 3 consecutive heartbeats are missed (30 seconds), the peer is marked as dead, preventing routing requests to an unresponsive node.

### 2. Distributed Rate Limiting (AP Architecture)
- **Mechanism:** When Router A receives a request, it increments its local IP counter and fires an asynchronous background broadcast to sync the count with Router B.
- **Tradeoff (Eventual Consistency):** Ketu prioritizes Availability and Partition Tolerance (AP) over strict consistency. Syncing rate limits is non-blocking (fire and forget), ensuring zero latency penalty on client requests while maintaining eventual consistency across nodes.

### 3. Split-Brain Fallback
- **Mechanism:** If the network link between peers drops, each router automatically degrades to independent local enforcement. Rather than failing or blocking traffic, both nodes enforce rate limits (10 req/min) on their local view until connection is restored.

### 4. Cross-Node Failover & Loop Prevention
- **Mechanism:** If all local backends configured on Router A fail or time out, Router A attempts a cross-node failover by forwarding the request to Router B.
- **Loop Prevention:** To prevent infinite routing loops ("ping-ponging" between nodes when all cluster backends are down), Router A injects an `x-ketu-forwarded: true` header into the request. If Router B receives a request with this header and its own local backends also fail, it terminates the chain immediately and returns an HTTP error.

### 5. Dynamic Leader Promotion
- **Mechanism:** On startup, a node launched without a `--peer` flag assumes the `Leader` role, while nodes joining via `--peer` start as `Follower`s.
- **Self-Healing:** If the active Leader dies, the Follower's heartbeat monitor detects the loss after 3 missed pings and autonomously promotes itself to Leader (`leader lost, promoting self`), eliminating a single point of failure for cluster-wide background tasks.


## How to Run It

**1. Install prerequisites**
- [Rust](https://rustup.rs/)
- [Ollama](https://ollama.com/)

**2. Pull the models Ketu uses**
```bash
ollama pull llama3.2:3b
ollama pull qwen2.5:7b
```

**3. Start two Ollama instances, on separate ports**
```bash
# Terminal 1 — default port 11434
ollama serve

# Terminal 2 — port 11435
$env:OLLAMA_HOST="127.0.0.1:11435"; ollama serve
```

**4. Create a `config.toml` in the project root**
```toml
[[backends]]
url = "http://localhost:11434"
model = "llama3.2:3b"
weight = 70

[[backends]]
url = "http://localhost:11435"
model = "qwen2.5:7b"
weight = 30
```

**5. Run Ketu**
```bash
cargo run
```

**6. Send a test request**
Using Bruno, curl, or any HTTP client, send a `POST` request to `http://localhost:3000/route`:

```json
{
  "model": "llama3.2:3b",
  "prompt": "hello"
}
```
A successful response returns the model's generated output as JSON.

**6. Run a Multi-Node Cluster (Phase 2)**

Start the first node (Leader):
```bash
cargo run -- --port 3000
```
In a second terminal, start the peer node (Follower):
```bash 
cargo run -- --port 3001 --peer 127.0.0.1:3000
```

## Endpoints

### `GET /health`
Returns `200 OK` with the text `"ok"`. Confirms Ketu's own server is running — does not check whether the underlying Ollama backends are reachable.

### `POST /route`
Accepts a JSON body:
```json
{
  "model": "llama3.2:3b",
  "prompt": "your prompt here"
}
```
- `prompt` — the text sent to the selected backend.
- `model` — currently ignored by the router. Reserved for future semantic routing (selecting a backend based on prompt content/complexity rather than round-robin/weighting).

The backend is chosen automatically via weighted round-robin, with automatic retry/failover (up to 2 attempts) if the selected backend fails or times out.

Returns:
```json
{
  "response": "the model's generated output"
}
```

### `GET /stats`
Returns a JSON object mapping each backend URL to its cumulative token count:
```json
{
  "http://localhost:11434": 1234,
  "http://localhost:11435": 567
}
```
Latency is tracked and logged per-request to the terminal but not currently exposed via this endpoint.

### `x-ketu-forwarded` 
Internal header attached automatically during cross-node failover to prevent infinite loops between peer nodes.


## Known Limitations

- The `model` field accepted in `/route` requests is not currently used to select a backend; routing is decided entirely by round-robin/weighting. The field is kept as a placeholder for future semantic routing (choosing a backend based on the prompt itself, rather than blind rotation).
- Retry attempts on failover are capped at 2 total. Since the pool currently only has 2 backends, there's no distinct 3rd backend to fall back to if both fail.
- Latency is tracked and logged per-request to the terminal, but not yet aggregated or exposed through the `/stats` endpoint.

## Benchmarks

### System Specs
* **CPU:** AMD Ryzen 7 7435HS 3.1 GHz
* **GPU:** NVIDIA RTX 4050 Laptop GPU (6GB VRAM)
* **RAM:** 24GB DDR5 4800MHz
* **Models:** llama3.2:3b, qwen2.5:7b (via Ollama)

### Methodology
Ran 100 requests through Ketu using a Python script (`benchmark.py`), alternating short and long prompts, with a rate-limit burst included partway through the run. *Test conditions: deliberately run while on a Google Meet call with 20 participants (screen sharing, no video) to simulate real-world background load rather than testing under idle conditions.*

### Results
* **Average latency (successful requests):** 7,530.77 ms
* **Error rate:** 15% (15 of 100 requests)
* **Average response length (status 200):** 1,429.02 characters
* **Average response length (status 0):** 9.00 characters

*(Check `benchmark_results1.csv` for raw data)*

> **Architectural Insight on Error Rate:**
> These 15% were not real failures in Ketu. The benchmark script's own client-side timeout was set to 15 seconds, while Ketu's internal timeout is 30 seconds. Under load, some long-prompt responses took longer than 15 seconds to generate; the script closed the connection before Ketu could finish, even though Ketu itself was still working correctly within its own timeout window. This surfaced a real lesson: if the tool measuring a system has a shorter timeout than the system being measured, there's a good chance the system's actual responses won't be logged or collected at all, making the system look like it's failing when it isn't.

### Distributed Benchmarks (Phase 2 Crucible Test)
To prove the resilience of the multi-node cluster, 200 requests were fired randomly across both Router A and Router B. Halfway through the test (~request 100), one of the two underlying Ollama backends was forcefully terminated (`Ctrl+C`) to simulate a critical hardware failure.

**Results:**
* **Average latency:** 9,502.00 ms (Up from 7,530 ms)
* **Error rate:** 0.00% (Down from 15%)
* **Average response length:** 1,628.29 characters

*(Check `benchmark_results2.csv` for raw data)*

> **Architectural Insight on Failover Tax:** 
> The error rate dropped to an absolute zero, proving that cross-node failover works seamlessly, not a single client request was dropped despite a hard backend crash. The tradeoff is the latency increase. Average latency rose by ~2 seconds because the cluster lost 50% of its compute capacity mid-test. The surviving backend had to process the remaining queue entirely on its own, demonstrating that in distributed systems, resilience often costs performance.