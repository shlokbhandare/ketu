import csv
import json
import random
import statistics
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import List, Dict, Any

DEFAULT_URLS = ["http://localhost:3000/route", "http://localhost:3001/route"]
DEFAULT_OUTPUT = "benchmark_results.csv"
TOTAL_REQUESTS = 200
BURST_START = 45
BURST_COUNT = 15
REGULAR_DELAY_SECONDS = 0.05
REQUEST_TIMEOUT_SECONDS = 45

SHORT_PROMPTS = [
    "hello",
    "summarize",
    "test run",
    "rust code",
    "json body",
    "route check",
    "ai status",
    "debug this",
    "quick ping",
    "health ok",
]

LONG_PROMPTS = [
    "Write a concise explanation of how a small local API router can balance traffic across multiple backend services while keeping latency predictable under bursty load.",
    "Describe the tradeoffs between simple round robin scheduling and weighted routing when one backend is faster but less reliable than another, especially in a request-per-minute limited environment.",
    "Explain why alternating prompt lengths during a benchmark can reduce the chance that repeated content causes misleading performance results, and how this helps surface rate limiting behavior more clearly.",
    "Imagine a small orchestration service receiving mixed short and long user requests and discuss how prompt complexity can affect both token usage and response generation time in practice.",
    "Provide a short analysis of the relationship between burst traffic, rate limiting, and observable error rates when many requests arrive within a very short window.",
]


def build_payload(prompt: str) -> Dict[str, Any]:
    return {"model": "llama3.2:3b", "prompt": prompt}


def pick_prompt(index: int) -> str:
    if index % 2 == 0:
        return SHORT_PROMPTS[index % len(SHORT_PROMPTS)]
    return LONG_PROMPTS[index % len(LONG_PROMPTS)]


def send_request(url: str, prompt: str) -> Dict[str, Any]:
    payload = build_payload(prompt)
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )

    start = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=REQUEST_TIMEOUT_SECONDS) as response:
            body = response.read()
            status_code = response.getcode()
    except urllib.error.HTTPError as exc:
        body = exc.read()
        status_code = exc.code
    except Exception as exc:
        body = str(exc).encode("utf-8")
        status_code = 0

    latency_ms = round((time.perf_counter() - start) * 1000, 2)
    return {
        "status_code": status_code,
        "latency_ms": latency_ms,
        "response_length": len(body),
        "prompt_type": "short" if prompt in SHORT_PROMPTS else "long",
        "prompt": prompt,
    }


def write_results(results: List[Dict[str, Any]], output_path: str) -> None:
    output = Path(output_path)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=["index", "status_code", "latency_ms", "response_length", "prompt_type", "url", "prompt"],
        )
        writer.writeheader()
        for idx, row in enumerate(results):
            writer.writerow({"index": idx + 1, **row})


def summarize(results: List[Dict[str, Any]]) -> None:
    if not results:
        print("No requests were completed.")
        return

    two_hundred_status = [row for row in results if row["status_code"] == 200]
    non_two_hundred = [row for row in results if row["status_code"] != 200]  

    if two_hundred_status:
        avg_latency = statistics.mean(row["latency_ms"] for row in two_hundred_status)
        print(f"Average latency across 200-status requests: {avg_latency:.2f} ms")
    else:
        print("Average latency across 200-status requests: n/a")

    error_rate = (len(non_two_hundred) / len(results)) * 100 if results else 0.0
    print(f"Error rate (non-200 responses): {error_rate:.2f}%")

    by_status: Dict[int, List[float]] = {}
    for row in results:
        by_status.setdefault(row["status_code"], []).append(row["response_length"])

    print("Average response length by status code:")
    for status_code in sorted(by_status):
        avg_length = statistics.mean(by_status[status_code])
        print(f"  {status_code}: {avg_length:.2f}")


def main() -> None:
    output_path = DEFAULT_OUTPUT
    print(f"Sending {TOTAL_REQUESTS} requests across {DEFAULT_URLS}")
    print(f"Results will be saved to {output_path}")

    results: List[Dict[str, Any]] = []
    for idx in range(TOTAL_REQUESTS):
        prompt = pick_prompt(idx)
        url = random.choice(DEFAULT_URLS)

        if BURST_START <= idx < BURST_START + BURST_COUNT:
            if idx == BURST_START:
                print("Starting burst of rapid requests...")
            result = send_request(url, prompt)
            result["index"] = idx + 1
            result["url"] = url
            results.append(result)
            continue

        result = send_request(url, prompt)
        result["index"] = idx + 1
        result["url"] = url
        results.append(result)
        if idx < TOTAL_REQUESTS - 1:
            time.sleep(REGULAR_DELAY_SECONDS)

    write_results(results, output_path)
    summarize(results)
    print(f"Raw results written to {output_path}")


if __name__ == "__main__":
    main()
