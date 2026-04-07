use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use proxy::state::AppState;
use std::sync::Arc;
use std::thread;

fn bench_rate_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limit");

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("check_rate_limit", format!("{}_threads", num_threads)),
            &num_threads,
            |b, &n_threads| {
                let state = Arc::new(AppState::new(vec!["http://127.0.0.1:8080".into()]));

                b.iter(|| {
                    let mut handles = vec![];
                    for t in 0..n_threads {
                        let s = state.clone();
                        handles.push(thread::spawn(move || {
                            for i in 0..10_000 {
                                let ip = format!("10.0.{}.{}", t, i % 256);
                                let _ = s.check_rate_limit(&ip);
                            }
                        }));
                    }
                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_rate_limit);
criterion_main!(benches);
