pub mod handler;
pub mod state;
pub mod workers;

use axum::Router;
use handler::proxy_handler;
use state::AppState;

pub fn build_app(state: AppState) -> Router {
    Router::new().fallback(proxy_handler).with_state(state)
}
