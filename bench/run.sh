#!/bin/bash
set -e

DURATION="30s"
CONCURRENCIES=(1 10 50 100 200 500)
RESULTS_DIR="bench/results"
mkdir -p "$RESULTS_DIR"

# Build mock once
cargo build -p upstream_mock --release

for MODE in "striped" "single"; do
  echo "=== Building $MODE mode ==="
  if [ "$MODE" = "single" ]; then
    cargo build -p proxy --release --features "bench single-lock"
  else
    cargo build -p proxy --release --features bench
  fi

  # Start upstream mocks
  ./target/release/upstream_mock &
  MOCK_PID=$!
  sleep 2
  curl -s http://127.0.0.1:8080 >/dev/null || {
    echo "Mocks failed to start"
    exit 1
  }

  # Start proxy
  ./target/release/proxy &
  PROXY_PID=$!
  sleep 2
  curl -s http://127.0.0.1:3000 >/dev/null || {
    echo "Proxy failed to start"
    exit 1
  }

  for C in "${CONCURRENCIES[@]}"; do
    echo "  Benchmarking $MODE @ $C connections..."
    # Warmup
    oha -c "$C" -z 5s --no-tui http://127.0.0.1:3000 >/dev/null 2>&1

    # Actual run
    oha -c "$C" -z "$DURATION" --output-format json http://127.0.0.1:3000 \
      >"$RESULTS_DIR/${MODE}_c${C}.json" 2>/dev/null
  done

  kill $PROXY_PID $MOCK_PID 2>/dev/null
  wait $PROXY_PID $MOCK_PID 2>/dev/null || true
  sleep 1
done

echo "Done. Results in $RESULTS_DIR/"
