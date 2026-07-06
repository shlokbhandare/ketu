# Ketu

Ketu sits between users and multiple local AI backends, deciding which model should handle each incoming request. It tracks the number of requests per minute and the latency of every request, giving visibility into how the system is performing under load.

## How This Was Built

I (Shlok) am the Architect, assissted by Claude as a mentor, designing every component, data flow, and technical decision. AI (Cursor/Continue.dev) is the Builder, generating the Rust syntax. Every architectural decision in this project can be explained and defended by without relying on the AI that helped write it or the AI that helped plan it.

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
│     BackendPool     │◄───────────────────────---──┐  round-robin + weighted
└──────────┬──────────┘                             │  selection
           │                                        │
           ▼                                        │
┌─────────────────────┐                             │
│      ollama.rs      │  forwards request           │
└──────────┬──────────┘                             │
           │                                        │
           ▼                                        │
┌─────────────────────┐                             │
│   Ollama Backend    │  generates response         │
└──────────┬──────────┘                             │
           │                                        │
           ├── fails / timeout (max 2 attempts) ────┘
           │
           │ success
           ▼
┌─────────────────────┐
│      Response       │──► Client
└─────────────────────┘
```

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

## Known Limitations

- The `model` field accepted in `/route` requests is not currently used to select a backend, routing is decided entirely by round-robin/weighting. The field is kept as a placeholder for future semantic routing (choosing a backend based on the prompt itself, rather than blind rotation).
- Retry attempts on failover are capped at 2 total, since the pool currently only has 2 backends, there's no distinct 3rd backend to fall back to if both fail.
- Latency is tracked and logged per-request to the terminal, but not yet aggregated or exposed through the `/stats` endpoint.