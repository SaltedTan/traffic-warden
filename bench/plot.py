import json, glob, re
import matplotlib.pyplot as plt

# --- Parse oha results ---
modes = {"striped": {}, "single": {}}

for path in glob.glob("bench/results/*.json"):
    name = path.split("/")[-1].replace(".json", "")
    mode, conc = name.rsplit("_c", 1)
    conc = int(conc)
    with open(path) as f:
        data = json.load(f)
    modes[mode][conc] = {
        "rps": data["summary"]["requestsPerSec"],
        "p50": data["latencyPercentiles"]["p50"],
        "p99": data["latencyPercentiles"]["p99"],
    }

concurrencies = sorted(modes["striped"].keys())

# --- Parse Criterion results ---
def parse_criterion(filepath):
    results = {}
    current_bench = None
    with open(filepath) as f:
        for line in f:
            # Match benchmark name like "rate_limit/check_rate_limit/4_threads"
            m = re.search(r"check_rate_limit/(\d+)_threads", line)
            if m:
                current_bench = int(m.group(1))
            # Match time line like "time:   [1.2345 ms 1.3456 ms 1.4567 ms]"
            m = re.search(r"time:\s+\[[\d.]+ \w+\s+([\d.]+) (\w+)", line)
            if m and current_bench is not None:
                value = float(m.group(1))
                unit = m.group(2)
                if unit == "ms":
                    value *= 1.0
                elif unit == "µs" or unit == "us":
                    value /= 1000.0
                elif unit == "s":
                    value *= 1000.0
                results[current_bench] = value
                current_bench = None
    return results

criterion_striped = parse_criterion("bench/results/criterion_striped.txt")
criterion_single = parse_criterion("bench/results/criterion_single.txt")

# --- Plot ---
fig, (ax1, ax2, ax3) = plt.subplots(1, 3, figsize=(18, 5))

# Chart 1: Throughput (oha)
for mode, label, color in [
    ("striped", "Lock-Striped (64 shards)", "#2196F3"),
    ("single", "Single Mutex", "#F44336"),
]:
    rps = [modes[mode][c]["rps"] for c in concurrencies]
    ax1.plot(concurrencies, rps, marker="o", label=label, color=color)
ax1.set_xlabel("Concurrent Connections")
ax1.set_ylabel("Requests/sec")
ax1.set_title("System Throughput (oha)")
ax1.legend()
ax1.grid(True, alpha=0.3)

# Chart 2: Tail latency (oha)
for mode, label, color in [
    ("striped", "Lock-Striped (64 shards)", "#2196F3"),
    ("single", "Single Mutex", "#F44336"),
]:
    p99 = [modes[mode][c]["p99"] * 1000 for c in concurrencies]
    ax2.plot(concurrencies, p99, marker="o", label=label, color=color)
ax2.set_xlabel("Concurrent Connections")
ax2.set_ylabel("p99 Latency (ms)")
ax2.set_title(" System Tail Latency (oha)")
ax2.legend()
ax2.grid(True, alpha=0.3)

# Chart 3: Criterion microbenchmark (bar chart)
threads = sorted(criterion_striped.keys())
x = range(len(threads))
width = 0.35
bars1 = ax3.bar([i - width/2 for i in x],
                [criterion_striped[t] for t in threads],
                width, label="Lock-Striped (64 shards)", color="#2196F3")
bars2 = ax3.bar([i + width/2 for i in x],
                [criterion_single[t] for t in threads],
                width, label="Single Mutex", color="#F44336")
ax3.set_xlabel("Threads")
ax3.set_ylabel("Time (ms)")
ax3.set_title("Rate Limiter Microbenchmark")
ax3.set_xticks(list(x))
ax3.set_xticklabels([str(t) for t in threads])
ax3.legend()
ax3.grid(True, alpha=0.3, axis="y")

plt.tight_layout()
plt.savefig("bench/results/benchmark.png", dpi=150)
print("Saved bench/results/benchmark.png")
