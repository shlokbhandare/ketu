# Build Log

## July 27, 2026

**feat: implement heuristic semantic routing engine**
Description:

> Replaced blind round-robin routing with an intelligent heuristic scoring engine (`analyzer.rs`). Ketu now evaluates prompt complexity in microseconds using keyword density, structural patterns, and string length. Simple prompts are routed to the Low-Latency backend (llama3.2:3b), while complex prompts (code/math) are automatically routed to the High-Capacity backend (qwen2.5:7b). Explicitly chose a CPU-bound heuristic model over vector embeddings to preserve the sub-millisecond latency requirements of an API Gateway.

**fix: rate limit sweep, peer sync guard, reqwest reuse, test coverage, dead code removal**
Description:

> Technical debt cleanup before Phase 3. Moved the O(N) HashMap cleanup to a background tokio task, prevented orphaned task leaks by guarding peer syncs, reused reqwest clients to preserve connection pooling during failovers, and enforced 'fail fast' memory allocation for rate-limited requests.

## July 25, 2026

**docs: distributed system benchmarks**
Description:

> Latency increased by ~2 seconds because losing a backend mid-test forced the surviving backend to handle double the load. However, the system became infinitely more resilient—the error rate dropped to 0.00% because the cross-node failover caught every single request destined for the dead backend.

> Compared to Phase 1's single-instance benchmark, distributed latency increased from ~7.5s to 9.5s because losing a backend mid-test forced the survivor to handle double the load. However, resilience drastically improved—the error rate dropped from 15% to 0.00% because cross-node failover successfully caught every dropped request.

## July 20, 2026

**feat: dynamic leader promotion on failure**
Description:

> A single point of failure is a critical joint in software that is so inseparable from the process that if it fails, the entire program collapses. Yes, I just eliminated one, if the static leader dies, the follower now dynamically promotes itself to keep the cluster alive.

**feat: split-brain fallback and client timeout**
Description:

> Split-brain occurs when the network link between nodes drops, but both stay online and process traffic independently, causing data divergence (e.g., allowing 20 total requests instead of 10). By relying on local enforcement and adding a 2-second timeout to the HTTP client, we chose AP (Availability and Partition Tolerance) from the CAP theorem, sacrificing perfect Consistency to keep the routers from hanging or crashing.## July 20, 2026

**feat: distributed rate limiting across instances**
Description:

> We made a consistency tradeoff by using fire-and-forget deltas instead of blocking the main CPU thread to maintain a perfectly synchronized shared limit. This isn't perfect, a user could squeeze in an extra 1-2 requests (e.g., 11 or 12 on a limit of 10) by simultaneously spamming both routers before the network broadcasts propagate.

## July 18, 2026

**feat: peer heartbeat and dead peer detection**
Description:

> A heartbeat in distributed systems is a regulated signal sent between online instances. When one server detects the consecutive absence of that signal, it concludes the instance is down and logs the failure. Systems like Kubernetes and Kafka use this exact feature.

## July 13, 2026

**feat: distributed backend health sharing**
Description:

> When Router A detects a slow backend, there is a small window before Router B receives the update where they don't share the exact same view. However, they eventually reach coordination. This is eventual consistency, and Ketu definitely has it.

## July 6, 2026

**feat: peer discovery on startup**
Description:

>Multiple router instances allow horizontal scaling and provide redundancy, and set up the system for future global expansion. A single router instance is a single point of failure, if it goes down, all traffic routed through it dies with it. Multiple instances mean the routing layer itself survives a failure, not just the backend layer.

## July 4, 2026

**feat: automatic failover with retry logic**
Description:

>Timeout is a termination of client access to a backend if the backend is taking too long to respond. 30s is used here to accommodate my own hardware limitations, which cause AI model cold starts to run upwards of 20 seconds.
>Failover is the transfer of operations to another backend if one backend is unable to respond for any reason. It matters in production as, in the real world, quite a lot of causative factors can cause failure in operations, hence a backup and the logic to use it is necessary.
*Known limitation:* With only 2 backends, retries are capped at 2 total attempts, there's no distinct 3rd backend to fall back to, so the retry loop can't go for a 3rd try.

**fix: prune stale rate-limit entries on request**
Description:
>Fixed known limitaion from june 29, 2026.

## June 29, 2026

**feat: /stats endpoint with token counts and latency**
Description:

>Token counts are tracked to ensure fair and limited usage of servers and models, stabilizing computing costs.
>Inference cost is the cost of all resources consumed per token, GPU compute, memory, electricity, and time. The company running the model pays these costs and recoups them by charging users per token, making token tracking essential for billing and cost attribution.

**feat: per-backend latency tracking**
Description:

>Latency is the time taken for a request to be processed from send to response. 
>p99 latency is the latency that 99% of requests fall under, meaning 1% took longer. In production, that 1% of slow requests is what users complain about, hence it matters for AI inference where cold starts (like Ollama's first load at 5044ms) can spike p99 significantly.

**feat: rate limiting with 429 enforcement**
Description:

>HTTP 429 is a status code meaning 'too many requests' returned when a client exceeds the allowed request limit. The body can be empty or contain an error message depending on implementation. 
>Rate limiting matters in production AI systems as it prevents unexpected overloading or influx of requests sent by users with malicious intent, protecting backend resources and ensuring fair usage across all clients.
*Known limitation:* HashMap grows unbounded, IP entries are never deleted after their window expires. 
A malicious actor could exhaust memory by spoofing millions of unique IPs (DoS vector). 
Fix: periodic sweep to delete stale entries. To be addressed in a later session.

## June 28, 2026
**feat: per-ip request counter**
Description:

>HashMap stores data as key-value pairs, like a storage locker rather than a bag of valuables, unlike a Vec which is just positional. 
>IP-based tracking ensures purity in rate limiting. IP addresses are tougher to fake and provide more reliable security compared to tracking by username or session.

## June 27, 2026
**feat: weighted routing with config file**
Description:

>Weighted routing: used when one server is bigger and can handle much more requests compared to another. If both weights are equal, it equally splits the traffic, effectively normal round robin. 
>Real world: Nginx has a `weight` parameter in its upstream config with the same concept; AWS ALB (Application Load Balancer) also does weighted target groups.

## June 26, 2026
**feat: round-robin load balancing across backends**
Description:

>Round-robin: a cycling alternating algorithm, like dealing cards to players, each backend gets a request in turn regardless of anything else. Alternative: least latency routing, instead of blind alternation, the request goes to whichever backend is currently responding fastest. Used when backends have unequal performance and sending to a slow one wastes time.


## June 23, 2026
**feat: backend pool struct**
Description:
>Use of a struct here: Allowed the function of addition and subtraction of more models for use and calling, thereby adding the first instance of actionable scalability in this project.

## June 15, 2026
**feat: router forwards prompts to ollama backend**

Description:
> What is happening in the backend currently: Bruno sends the POST request, Axum receives the request, and hands off the parsed JSON (RouteRequest struct) to the route function, which then calls ollama:generate. Then it receives the output, and Ketu returns it as an HTTP request back to Bruno

**feat: ollama client function stub**

Description:
- reqwest vs. axum: reqwest allows Ketu to act like a client and send data to models (in this case). axum allows ketu to act as a server for accepting data from users, and sending them off to reqwest
- Learnt what classifies as 'manipulation of data' and what doesn't.

---

## June 8, 2026
**feat: /route endpoint accepts and echoes JSON**

Description:
- GET and POST requests: GET request is to retrieve data, or to literally 'get' while POST request is to send a payload to a server to manipulate.
- JSON: A lightweight data text format, a universal shipping container for transport of data between programs, languages etc.

**feat: basic health check endpoint**

Description:
- Async: a pattern that allows a program to pause and handle other incoming requests while waiting for a slow network response instead of freezing.
- Tokio: The async runtime or engine that keeps track of all these paused tasks and resumes them exactly where they were left off once the data arrives
- Benefits/uses: Allows a single CPU thread to juggle thousands of concurrent requests efficiently without crashing the system due to memory overload

---

## June 6, 2026
**README update**

Update README.md

**init: project scaffold and README**

Initial commit