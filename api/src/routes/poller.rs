//! Poller status HTTP endpoint.
//!
//! GET /api/v1/poller/status — returns the current state of the background
//! forecast poller as JSON.

use axum::extract::State;
use axum::Json;

use crate::services::poller::{PollerState, SharedPollerState};

/// Get the current poller status.
///
/// Returns per-checkpoint info (expires_at, last_fetched_at, last_model_run_at,
/// last_poll_result) and global info (next_wakeup_at, last_poll_completed_at, active).
#[utoipa::path(
    get,
    path = "/api/v1/poller/status",
    tag = "Poller",
    responses(
        (status = 200, description = "Current poller status", body = PollerState),
    )
)]
pub async fn get_poller_status(State(state): State<SharedPollerState>) -> Json<PollerState> {
    let s = state.read().await;
    Json(s.clone())
}
