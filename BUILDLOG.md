# Build Log

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