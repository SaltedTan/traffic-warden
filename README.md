# Traffic Warden

A lightweight, high-performance, and thread-safe HTTP reverse proxy built in Rust. 

Traffic Warden acts as a defensive layer for upstream services, implementing a concurrent, fixed-window rate limiter with automated memory management. It is designed to handle high-throughput traffic without succumbing to data races, socket exhaustion, or out-of-memory (OOM) leaks.



## Architecture & Workspace

This project is structured as a Cargo Workspace containing two distinct services:
1. **`proxy`**: The core reverse proxy service implementing Axum, Tokio, and thread-safe state.
2. **`upstream_mock`**: A lightweight HTTP backend server used to validate proxy forwarding.

## Key Features

* **Thread-Safe State Management:** Tracks concurrent client IPs and request counts using an `Arc<Mutex<HashMap>>`.
* **Fixed-Window Rate Limiting:** Restricts clients to a configurable number of requests per 60-second sliding window (returns `429 Too Many Requests`).
* **Asynchronous Garbage Collection:** A detached `tokio::spawn` background worker wakes up periodically to safely sweep and drop expired IP allocations, preventing memory leaks over time.
* **Connection Pooling:** Reuses a single `reqwest::Client` internal pool to prevent ephemeral TCP socket exhaustion under heavy load.
* **Defensive Edge Boundaries:** Implements strict 5MB payload size limits to mitigate malicious OOM attacks.

## Engineering Decisions & Trade-offs

Building a concurrent proxy requires strict adherence to memory and thread safety. Key decisions include:

* **Lexical Scoping for Locks:** To prevent deadlocks in the async runtime, the `MutexGuard` for the rate-limit state is explicitly confined to a lexical scope block. This guarantees the lock is dropped *before* the asynchronous `.await` boundary when calling the upstream server, keeping lock contention in the microsecond range.
* **In-Memory vs. Redis:** To demonstrate low-level systems synchronization, the rate limit state is held entirely in-memory using standard library synchronization primitives rather than offloading to an external Redis cluster.
* **Header Sanitization:** Strips internal `Host` headers before forwarding to ensure compatibility with strict upstream load balancers and ingress controllers.

## Quick Start

### Prerequisites
* Rust (stable) and Cargo

### Running the Environment

1. **Clone the repository:**
   ```bash
   git clone [https://github.com/yourusername/traffic-warden.git](https://github.com/yourusername/traffic-warden.git)
   cd traffic-warden
   ```

2. **Start the Upstream Mock Server:**
   Open a terminal and run the mock backend (listens on port 8080):
   ```bash
   cargo run -p upstream_mock
   ```

3. **Start the Traffic Warden Proxy:**
   Open a second terminal and run the proxy (listens on port 3000):
   ```bash
   cargo run -p proxy
   ```

4. **Test the Rate Limiter:**
   Hit the proxy with `curl`. By default, the limit is 5 requests per 60 seconds.
   ```bash
   # Run this 6 times rapidly
   curl -v http://127.0.0.1:3000
   ```
   *Requests 1-5 will return `200 OK` from the upstream.*
   
   *Request 6 will be intercepted and return `429 Too Many Requests`.*

## Built With

* [Rust](https://www.rust-lang.org/)
* [Tokio](https://tokio.rs/) - Asynchronous runtime
* [Axum](https://github.com/tokio-rs/axum) - Ergonomic and modular web framework
* [Reqwest](https://docs.rs/reqwest/latest/reqwest/) - HTTP Client
