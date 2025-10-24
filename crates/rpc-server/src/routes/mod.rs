use crate::state::AppState;
use axum::Router;

mod ops;
mod v1;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .nest("/v1", v1::create_router())
        .merge(ops::create_router())
        .with_state(state)
}
