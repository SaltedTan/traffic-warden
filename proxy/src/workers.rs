use crate::state::{AppState, CAPACITY, REFILL_RATE};
use std::time::{Duration, Instant};

pub fn spawn_background_workers(state: AppState) {
    // The Garbage Collector
    let garbage_collector_state = state.rate_limit.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let now = Instant::now();
            let max_age_secs = CAPACITY / REFILL_RATE;

            #[cfg(not(feature = "single-lock"))]
            {
                for shard in garbage_collector_state.iter() {
                    let mut map = shard.lock().unwrap();
                    map.retain(|_ip, state| {
                        (now - state.last_updated).as_secs_f32() <= max_age_secs
                    });
                    // The lock for this specific shard drops here, before moving to the next one
                }
            }

            #[cfg(feature = "single-lock")]
            {
                let mut map = garbage_collector_state.lock().unwrap();
                map.retain(|_ip, state| (now - state.last_updated).as_secs_f32() <= max_age_secs);
            }
        }
    });

    // The Active Heath Checker
    let health_checker_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;

            let mut new_healthy_list = Vec::new();

            for upstream in &health_checker_state.all_upstreams {
                let ping_url = format!("{}/", upstream);

                let result = health_checker_state
                    .client
                    .get(&ping_url)
                    .timeout(Duration::from_secs(2))
                    .send()
                    .await;

                if let Ok(res) = result
                    && res.status().is_success()
                {
                    new_healthy_list.push(upstream.clone());
                }
            }

            let mut current_healthy = health_checker_state.healthy_upstreams.write().unwrap();
            *current_healthy = new_healthy_list;
        }
    });
}
