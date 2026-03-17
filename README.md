# Traffic Warden

A lightweight, high-performance, and thread-safe HTTP reverse proxy built in Rust.

Traffic Warden acts as a defensive layer for upstream services, implementing a concurrent **Token Bucket** rate limiter with automated memory management. It is designed to handle high-throughput traffic without succumbing to data races, socket exhaustion, or out-of-memory (OOM) leaks.

## Architecture & Workspace

This project is structured as a Cargo Workspace containing two distinct services:

1. **`proxy`**: The core reverse proxy service implementing Axum, Tokio, and thread-safe state.
2. **`upstream_mock`**: A lightweight HTTP backend server used to validate proxy forwarding.

## Key Features

* **Thread-Safe State Management via Lock Striping:** Tracks concurrent client IPs and token balances using a custom sharded concurrent map (`Arc<[Mutex<HashMap>; 64]>`). Incoming requests are deterministically hashed and routed to specific shards, allowing true multi-core concurrent processing.
* * **Token Bucket Traffic Shaping:** Replaces naive fixed time-windows with a continuous Token Bucket algorithm using precise time-delta calculations (`f32`). This completely mitigates boundary-burst exploits.
* **Asynchronous Garbage Collection:** A detached `tokio::spawn` background worker wakes up periodically to sweep expired IP allocations. It iterates through the 64 shards sequentially, locking and cleaning one at a time to prevent global blocking and maintain a stable RAM footprint.
* * **Connection Pooling:** Reuses a single `reqwest::Client` internal pool to prevent ephemeral TCP socket exhaustion under heavy load.
* **Defensive Edge Boundaries:** Implements strict 5MB payload size limits to mitigate malicious OOM attacks.

## Engineering Decisions & Trade-offs

Building a concurrent proxy requires strict adherence to memory and thread safety. Key decisions include:

* **Lock Striping Over Global Locks or External Crates:** Instead of using a single global `Mutex` (which causes severe thread queuing under DDoS conditions) or outsourcing the problem to a black-box crate like `dashmap`, the rate limiter shards its state across 64 independent Mutexes. Using a deterministic IP hashing function, requests are routed to specific shards, reducing lock contention by ~98% and preventing the Tokio thread pool from blocking.
* **RAII and Safe Lock Release:** To prevent deadlocks in the async runtime, the `MutexGuard` for the rate-limit state is explicitly confined to a lexical scope block. We rely on Rust's `Drop` trait (Resource Acquisition Is Initialization) to implicitly and safely release locks during early `HTTP 429` returns, keeping lock contention in the microsecond range.
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
   Hit the proxy with `curl`. By default, the bucket has a capacity of **5 tokens** and refills at a rate of **1 token every 12 seconds**.

   ```bash
   # Run this 6 times rapidly
   curl -v http://127.0.0.1:3000
   ```

   *Requests 1-5 will consume the initial tokens and return `200 OK` from the upstream.*

   *Request 6 will be intercepted and return `429 Too Many Requests`.*

   *Wait 12 seconds, and exactly 1 new request request will be allowed through.*

## Built With

* [Rust](https://www.rust-lang.org/)
* [Tokio](https://tokio.rs/) - Asynchronous runtime
* [Axum](https://github.com/tokio-rs/axum) - Ergonomic and modular web framework
* [Reqwest](https://docs.rs/reqwest/latest/reqwest/) - HTTP Client
