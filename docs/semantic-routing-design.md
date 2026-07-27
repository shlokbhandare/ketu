# Semantic Routing Design Decision

## The Choice: Rule-Based Heuristic Engine

Ketu will use a **rule-based heuristic scoring engine** for prompt classification, not an embedding-based classifier.

## Why Not Embeddings

An embedding model converts a prompt into a vector and compares it against pre-labeled category vectors to determine complexity. This requires:

- An additional HTTP call to an embedding API or local model
- Extra GPU/CPU processing to generate the vector
- Extra memory to hold the embedding model alongside the inference models already loaded

On a 6GB VRAM laptop already running two 7B-class models, that overhead is not acceptable. More importantly, an API gateway's first job is zero added latency, every millisecond Ketu spends classifying a prompt is a millisecond the user waits before inference even starts. Embeddings add 20-50ms minimum. A rule-based engine adds under 0.1ms.

## How the Heuristic Scoring Engine Works

Every prompt is run through a fast scoring pass. Each matched rule adds points to a **Complexity Score**. No API calls, no model inference, pure string matching and regex.

### Scoring Rules

**Keyword Density**

- Code and logic keywords (`rust`, `python`, `sql`, `algorithm`, `function`, `macro`, `database`) → +2 points each
- Analysis keywords (`analyze`, `compare`, `tradeoffs`, `architect`, `explain`) → +1 point each

**Structural Patterns**

- Code blocks (backticks) → +2 points
- Data formats (`{`, `[`, XML tags) → +1 point
- Math or equations (`$`, `=`) → +1 point

**Length Thresholds**

- Prompt longer than 250 characters → +1 point
- Prompt longer than 1000 characters → +2 points

### Routing Decision

| Score | Backend | Reasoning |
| --- | --- | --- |
| ≥ 3 | `qwen2.5:7b` (high-capacity) | Prompt likely requires reasoning, code, or large context |
| < 3 | `llama3.2:3b` (low-latency) | Prompt is likely short and factual — smaller model is faster and sufficient |

**Why threshold 3?** A score of 2 is too easy to hit accidentally (one keyword match + length), producing too many false "complex" classifications that unnecessarily route simple prompts to the slower model. A score of 5 is too strict — genuinely complex prompts with mixed signals would be misclassified as simple. 3 is the balance point that requires at least two meaningful signals before escalating to qwen.

## Known Limitations

- A 10-word prompt like "Write a Rust macro to parse an AST" will correctly score high (keyword match). But a 500-word prompt of raw pasted text ending in "summarize this in one sentence" may score high on length alone, routing to qwen unnecessarily. Length is an imperfect proxy for complexity.
- Keywords are English-only. Non-English prompts will likely score low regardless of actual complexity.
- Threshold was chosen by reasoning, not empirical tuning. Session 30's benchmark will validate or challenge this number with real data.

## What This Trades Off

Rule-based routing is fast and cheap but brittle — it cannot learn from mistakes or handle novel prompt patterns it wasn't designed for. An embedding-based approach would generalise better but costs latency and hardware resources that are not available on this machine. The plan is to ship rule-based first, benchmark it in Session 30, and revisit embedding-based routing in college phase when hardware constraints are relaxed.
