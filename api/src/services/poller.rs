//! Background poller for proactive forecast fetching.
//!
//! Polls yr.no for all checkpoints of upcoming races on a schedule driven by
//! yr.no's `Expires` header. This ensures the `forecasts` table captures every
//! model run even when no users are actively calling the API.
//!
//! Architecture:
//! - Sleeps until the earliest `expires_at` across all polled checkpoints + buffer
//! - On wake: refreshes all checkpoints, extracts forecasts at realistic time bands
//! - Retries if yr.no returned 304 (same data, extended expiry) up to MAX_RETRIES
//! - State is in-memory (`Arc<RwLock<PollerState>>`); on restart, schedule
//!   reconstructs from `yr_responses.expires_at`

use chrono::{DateTime, Duration, Timelike, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::models::Checkpoint;
use crate::db::queries;
use crate::helpers::dec_to_f64;
use crate::services::forecast::{build_single_insert_params, ensure_yr_cache_fresh};
use crate::services::yr::{extract_forecasts_at_times, YrClient};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Slowest realistic pace for cross-country skiing (km/h).
const POLLER_MIN_SPEED_KMH: f64 = 10.0;

/// Fastest realistic pace for cross-country skiing (km/h).
const POLLER_MAX_SPEED_KMH: f64 = 30.0;

/// How far ahead to look for upcoming races (days).
const POLLER_LOOKAHEAD_DAYS: i64 = 10;

/// Buffer added after the earliest `expires_at` before waking (seconds).
const POLLER_WAKEUP_BUFFER_SECS: u64 = 30;

/// Minimum sleep duration between poll cycles (seconds).
const POLLER_MIN_SLEEP_SECS: u64 = 60;

/// Maximum sleep duration between poll cycles (seconds).
const POLLER_MAX_SLEEP_SECS: u64 = 1800;

/// Delay between retries when yr.no returns 304 (seconds).
const POLLER_RETRY_DELAY_SECS: u64 = 120;

/// Maximum retries when yr.no keeps returning 304 after expiry.
const POLLER_MAX_RETRIES: u32 = 5;

/// Fallback sleep when no upcoming races exist (seconds).
const POLLER_NO_RACES_SLEEP_SECS: u64 = 3600;

// ---------------------------------------------------------------------------
// Poller state (in-memory, shared via Arc<RwLock<>>)
// ---------------------------------------------------------------------------

/// Status of a single checkpoint's last poll attempt.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CheckpointPollStatus {
    pub checkpoint_id: Uuid,
    pub checkpoint_name: String,
    pub race_name: String,
    pub distance_km: f64,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_fetched_at: Option<DateTime<Utc>>,
    pub last_model_run_at: Option<DateTime<Utc>>,
    /// "new_data", "not_modified", "error", or "pending"
    pub last_poll_result: String,
    pub extraction_count: usize,
}

/// Global poller state, exposed via the status endpoint.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PollerState {
    pub active: bool,
    pub next_wakeup_at: Option<DateTime<Utc>>,
    pub last_poll_completed_at: Option<DateTime<Utc>>,
    pub last_poll_duration_ms: Option<u64>,
    pub total_polls: u64,
    pub checkpoints: Vec<CheckpointPollStatus>,
}

impl PollerState {
    pub fn new() -> Self {
        Self {
            active: true,
            next_wakeup_at: None,
            last_poll_completed_at: None,
            last_poll_duration_ms: None,
            total_polls: 0,
            checkpoints: Vec::new(),
        }
    }
}

/// Shared poller state handle.
pub type SharedPollerState = Arc<RwLock<PollerState>>;

// ---------------------------------------------------------------------------
// Time-band calculation
// ---------------------------------------------------------------------------

/// Compute the hourly forecast time slots that should be extracted for a
/// checkpoint, based on its distance from the race start and realistic
/// speed bounds.
///
/// Returns a sorted, deduplicated list of hourly UTC times.
pub fn compute_extraction_times(race_start: DateTime<Utc>, distance_km: f64) -> Vec<DateTime<Utc>> {
    if distance_km <= 0.0 {
        // Start checkpoint — extract at race start time (floored to hour)
        let start_hour = floor_to_hour(race_start);
        return vec![start_hour];
    }

    // Earliest arrival: fastest pace
    let earliest_hours = distance_km / POLLER_MAX_SPEED_KMH;
    // Latest arrival: slowest pace
    let latest_hours = distance_km / POLLER_MIN_SPEED_KMH;

    let earliest_arrival = race_start + Duration::seconds((earliest_hours * 3600.0) as i64);
    let latest_arrival = race_start + Duration::seconds((latest_hours * 3600.0) as i64);

    let first_slot = floor_to_hour(earliest_arrival);
    let last_slot = ceil_to_hour(latest_arrival);

    let mut times = Vec::new();
    let mut current = first_slot;
    while current <= last_slot {
        times.push(current);
        current += Duration::hours(1);
    }

    times
}

/// Floor a datetime to the start of its hour.
fn floor_to_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.date_naive()
        .and_hms_opt(dt.time().hour(), 0, 0)
        .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
        .unwrap_or(dt)
}

/// Ceil a datetime to the next hour (or same if already on the hour).
fn ceil_to_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
    if dt.time().minute() == 0 && dt.time().second() == 0 && dt.time().nanosecond() == 0 {
        dt
    } else {
        floor_to_hour(dt) + Duration::hours(1)
    }
}

// ---------------------------------------------------------------------------
// Main poller loop
// ---------------------------------------------------------------------------

/// Run the background poller. This function never returns (runs until process exit).
///
/// Should be spawned via `tokio::spawn(run_poller(...))`.
pub async fn run_poller(pool: PgPool, yr_client: YrClient, state: SharedPollerState) {
    tracing::info!("Background poller started");

    loop {
        let poll_start = Utc::now();

        // 1. Find upcoming races and their checkpoints
        let races = match queries::get_upcoming_races_with_checkpoints(&pool, POLLER_LOOKAHEAD_DAYS)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Poller: failed to query upcoming races: {}", e);
                sleep_secs(POLLER_MIN_SLEEP_SECS).await;
                continue;
            }
        };

        if races.is_empty() {
            handle_no_races(&state).await;
            sleep_secs(POLLER_NO_RACES_SLEEP_SECS).await;
            continue;
        }

        // 2. Build list of all checkpoints to poll
        let all_checkpoints = collect_checkpoints(&races);
        let checkpoint_ids: Vec<Uuid> = all_checkpoints.iter().map(|(cp, _, _)| cp.id).collect();

        // 3. Get pre-poll fetched_at for each checkpoint (to detect 304 vs new data)
        let pre_fetched_at = build_pre_fetched_map(&pool, &all_checkpoints).await;

        // 4. Refresh yr.no cache for all checkpoints
        let (mut checkpoint_statuses, any_got_304) =
            poll_all_checkpoints(&pool, &yr_client, &all_checkpoints, &pre_fetched_at).await;

        // 5. Publish intermediate state so the status endpoint is useful mid-cycle
        {
            let mut s = state.write().await;
            s.checkpoints = checkpoint_statuses.clone();
        }

        // 6. Retry logic — if we got 304s, wait and retry up to MAX_RETRIES
        if any_got_304 {
            retry_304_checkpoints(
                &pool,
                &yr_client,
                &all_checkpoints,
                &pre_fetched_at,
                &mut checkpoint_statuses,
                &state,
            )
            .await;
        }

        // 7–8. Compute next wakeup and update final state
        let sleep_duration = finalize_poll_cycle(
            &pool,
            &state,
            &checkpoint_ids,
            checkpoint_statuses,
            poll_start,
        )
        .await;

        sleep_secs(sleep_duration).await;
    }
}

/// Update state and sleep when no upcoming races exist.
async fn handle_no_races(state: &SharedPollerState) {
    tracing::debug!(
        "Poller: no upcoming races within {} days, sleeping {} seconds",
        POLLER_LOOKAHEAD_DAYS,
        POLLER_NO_RACES_SLEEP_SECS
    );
    let mut s = state.write().await;
    s.checkpoints.clear();
    s.next_wakeup_at = Some(Utc::now() + Duration::seconds(POLLER_NO_RACES_SLEEP_SECS as i64));
    s.last_poll_completed_at = Some(Utc::now());
}

/// Flatten races into `(Checkpoint, race_name, race_start)` tuples.
fn collect_checkpoints(
    races: &[queries::RaceWithCheckpoints],
) -> Vec<(Checkpoint, String, DateTime<Utc>)> {
    let mut all = Vec::new();
    for rwc in races {
        for cp in &rwc.checkpoints {
            all.push((cp.clone(), rwc.race.name.clone(), rwc.race.start_time));
        }
    }
    all
}

/// Build a map of checkpoint_id → pre-poll fetched_at for 304 detection.
async fn build_pre_fetched_map(
    pool: &PgPool,
    all_checkpoints: &[(Checkpoint, String, DateTime<Utc>)],
) -> std::collections::HashMap<Uuid, Option<DateTime<Utc>>> {
    let mut map = std::collections::HashMap::new();
    for (cp, _, _) in all_checkpoints {
        match queries::get_yr_cached_response_any(pool, cp.id).await {
            Ok(Some(cached)) => {
                map.insert(cp.id, Some(cached.fetched_at));
            }
            Ok(None) => {
                map.insert(cp.id, None);
            }
            Err(e) => {
                tracing::warn!(
                    "Poller: failed to get pre-poll cache for checkpoint {}: {}",
                    cp.id,
                    e
                );
                map.insert(cp.id, None);
            }
        }
    }
    map
}

/// Poll all checkpoints once, returning statuses and whether any got 304.
async fn poll_all_checkpoints(
    pool: &PgPool,
    yr_client: &YrClient,
    all_checkpoints: &[(Checkpoint, String, DateTime<Utc>)],
    pre_fetched_at: &std::collections::HashMap<Uuid, Option<DateTime<Utc>>>,
) -> (Vec<CheckpointPollStatus>, bool) {
    let mut statuses = Vec::with_capacity(all_checkpoints.len());
    let mut any_got_304 = false;

    for (cp, race_name, race_start) in all_checkpoints {
        let result = poll_single_checkpoint(pool, yr_client, cp, *race_start, pre_fetched_at).await;
        let status = build_poll_status(cp, race_name, result, &mut any_got_304);
        statuses.push(status);
    }

    (statuses, any_got_304)
}

/// Convert a `PollResult` into a `CheckpointPollStatus`.
fn build_poll_status(
    cp: &Checkpoint,
    race_name: &str,
    result: PollResult,
    any_got_304: &mut bool,
) -> CheckpointPollStatus {
    match result {
        PollResult::NewData {
            expires_at,
            fetched_at,
            model_run_at,
            extraction_count,
        } => CheckpointPollStatus {
            checkpoint_id: cp.id,
            checkpoint_name: cp.name.clone(),
            race_name: race_name.to_string(),
            distance_km: dec_to_f64(cp.distance_km),
            expires_at: Some(expires_at),
            last_fetched_at: Some(fetched_at),
            last_model_run_at: model_run_at,
            last_poll_result: "new_data".to_string(),
            extraction_count,
        },
        PollResult::NotModified {
            expires_at,
            fetched_at,
            model_run_at,
        } => {
            *any_got_304 = true;
            CheckpointPollStatus {
                checkpoint_id: cp.id,
                checkpoint_name: cp.name.clone(),
                race_name: race_name.to_string(),
                distance_km: dec_to_f64(cp.distance_km),
                expires_at: Some(expires_at),
                last_fetched_at: fetched_at,
                last_model_run_at: model_run_at,
                last_poll_result: "not_modified".to_string(),
                extraction_count: 0,
            }
        }
        PollResult::Error(msg) => CheckpointPollStatus {
            checkpoint_id: cp.id,
            checkpoint_name: cp.name.clone(),
            race_name: race_name.to_string(),
            distance_km: dec_to_f64(cp.distance_km),
            expires_at: None,
            last_fetched_at: None,
            last_model_run_at: None,
            last_poll_result: format!("error: {}", msg),
            extraction_count: 0,
        },
    }
}

/// Retry checkpoints that got 304 until all get new data or MAX_RETRIES.
async fn retry_304_checkpoints(
    pool: &PgPool,
    yr_client: &YrClient,
    all_checkpoints: &[(Checkpoint, String, DateTime<Utc>)],
    pre_fetched_at: &std::collections::HashMap<Uuid, Option<DateTime<Utc>>>,
    checkpoint_statuses: &mut [CheckpointPollStatus],
    state: &SharedPollerState,
) {
    for retry in 1..=POLLER_MAX_RETRIES {
        tracing::info!(
            "Poller: some checkpoints got 304, retry {}/{}",
            retry,
            POLLER_MAX_RETRIES
        );
        sleep_secs(POLLER_RETRY_DELAY_SECS).await;

        let mut still_304 = false;
        for (i, (cp, race_name, race_start)) in all_checkpoints.iter().enumerate() {
            if checkpoint_statuses[i].last_poll_result != "not_modified" {
                continue;
            }
            let result =
                poll_single_checkpoint(pool, yr_client, cp, *race_start, pre_fetched_at).await;
            match result {
                PollResult::NewData {
                    expires_at,
                    fetched_at,
                    model_run_at,
                    extraction_count,
                } => {
                    checkpoint_statuses[i] = CheckpointPollStatus {
                        checkpoint_id: cp.id,
                        checkpoint_name: cp.name.clone(),
                        race_name: race_name.clone(),
                        distance_km: dec_to_f64(cp.distance_km),
                        expires_at: Some(expires_at),
                        last_fetched_at: Some(fetched_at),
                        last_model_run_at: model_run_at,
                        last_poll_result: "new_data".to_string(),
                        extraction_count,
                    };
                }
                PollResult::NotModified { .. } => {
                    still_304 = true;
                }
                PollResult::Error(msg) => {
                    checkpoint_statuses[i].last_poll_result = format!("error: {}", msg);
                }
            }
        }

        // Update state after each retry pass
        {
            let mut s = state.write().await;
            s.checkpoints = checkpoint_statuses.to_vec();
        }

        if !still_304 {
            tracing::info!("Poller: all checkpoints got new data after retry {}", retry);
            break;
        }
    }
}

/// Compute next wakeup, update final state, and return the sleep duration in seconds.
async fn finalize_poll_cycle(
    pool: &PgPool,
    state: &SharedPollerState,
    checkpoint_ids: &[Uuid],
    checkpoint_statuses: Vec<CheckpointPollStatus>,
    poll_start: DateTime<Utc>,
) -> u64 {
    let earliest_expiry = match queries::get_earliest_expiry(pool, checkpoint_ids).await {
        Ok(Some(exp)) => exp,
        Ok(None) => Utc::now() + Duration::seconds(POLLER_MAX_SLEEP_SECS as i64),
        Err(e) => {
            tracing::error!("Poller: failed to query earliest expiry: {}", e);
            Utc::now() + Duration::seconds(POLLER_MAX_SLEEP_SECS as i64)
        }
    };

    let next_wakeup = earliest_expiry + Duration::seconds(POLLER_WAKEUP_BUFFER_SECS as i64);

    let sleep_duration = {
        let until_wakeup = (next_wakeup - Utc::now()).num_seconds().max(0) as u64;
        until_wakeup.clamp(POLLER_MIN_SLEEP_SECS, POLLER_MAX_SLEEP_SECS)
    };

    let poll_duration_ms = (Utc::now() - poll_start).num_milliseconds().max(0) as u64;

    {
        let mut s = state.write().await;
        s.checkpoints = checkpoint_statuses;
        s.next_wakeup_at = Some(Utc::now() + Duration::seconds(sleep_duration as i64));
        s.last_poll_completed_at = Some(Utc::now());
        s.last_poll_duration_ms = Some(poll_duration_ms);
        s.total_polls += 1;
    }

    tracing::info!(
        "Poller: cycle complete in {}ms, sleeping {}s (earliest expiry: {})",
        poll_duration_ms,
        sleep_duration,
        earliest_expiry,
    );

    sleep_duration
}

// ---------------------------------------------------------------------------
// Single-checkpoint poll
// ---------------------------------------------------------------------------

enum PollResult {
    NewData {
        expires_at: DateTime<Utc>,
        fetched_at: DateTime<Utc>,
        model_run_at: Option<DateTime<Utc>>,
        extraction_count: usize,
    },
    NotModified {
        expires_at: DateTime<Utc>,
        fetched_at: Option<DateTime<Utc>>,
        model_run_at: Option<DateTime<Utc>>,
    },
    Error(String),
}

async fn poll_single_checkpoint(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoint: &Checkpoint,
    race_start: DateTime<Utc>,
    pre_fetched_at: &std::collections::HashMap<Uuid, Option<DateTime<Utc>>>,
) -> PollResult {
    // Step 1: Ensure yr.no cache is fresh
    let raw_json = match ensure_yr_cache_fresh(pool, yr_client, checkpoint).await {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!(
                "Poller: failed to refresh checkpoint {} ({}): {}",
                checkpoint.id,
                checkpoint.name,
                e,
            );
            return PollResult::Error(e.to_string());
        }
    };

    // Step 2: Check if we got genuinely new data by comparing fetched_at
    let post_cache = match queries::get_yr_cached_response_any(pool, checkpoint.id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return PollResult::Error("Cache row missing after refresh".to_string());
        }
        Err(e) => {
            return PollResult::Error(format!("DB error checking cache: {}", e));
        }
    };

    let pre = pre_fetched_at.get(&checkpoint.id).copied().flatten();
    let got_new_data = match pre {
        Some(pre_ts) => post_cache.fetched_at != pre_ts,
        None => true, // No prior cache = definitely new
    };

    if !got_new_data {
        // yr.no returned 304 — same data, possibly extended expiry
        // Extract model_run_at from the existing cached JSON
        let model_run_at = extract_model_run_at(&raw_json);
        return PollResult::NotModified {
            expires_at: post_cache.expires_at,
            fetched_at: Some(post_cache.fetched_at),
            model_run_at,
        };
    }

    // Step 3: Extract forecasts at realistic time bands
    let distance_km = dec_to_f64(checkpoint.distance_km);
    let extraction_times = compute_extraction_times(race_start, distance_km);

    if extraction_times.is_empty() {
        return PollResult::NewData {
            expires_at: post_cache.expires_at,
            fetched_at: post_cache.fetched_at,
            model_run_at: extract_model_run_at(&raw_json),
            extraction_count: 0,
        };
    }

    let extraction_result = match extract_forecasts_at_times(raw_json.clone(), &extraction_times) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                "Poller: extraction failed for checkpoint {} ({}): {}",
                checkpoint.id,
                checkpoint.name,
                e,
            );
            return PollResult::Error(format!("Extraction error: {}", e));
        }
    };

    // Step 4: Write extracted forecasts to DB
    let mut insert_count = 0;
    let fetched_at = post_cache.fetched_at;
    for parsed in extraction_result.forecasts.iter().flatten() {
        let params = build_single_insert_params(checkpoint.id, parsed, fetched_at);
        match queries::insert_forecast(pool, params).await {
            Ok(Some(_)) => insert_count += 1, // New row inserted
            Ok(None) => {}                    // Deduplicated (already existed)
            Err(e) => {
                tracing::warn!(
                    "Poller: failed to insert forecast for checkpoint {} at {}: {}",
                    checkpoint.id,
                    parsed.forecast_time,
                    e,
                );
            }
        }
    }

    let model_run_at = extract_model_run_at(&raw_json);

    tracing::debug!(
        "Poller: checkpoint {} ({}) — extracted {}/{} time slots, inserted {} new rows",
        checkpoint.id,
        checkpoint.name,
        extraction_result
            .forecasts
            .iter()
            .filter(|f| f.is_some())
            .count(),
        extraction_times.len(),
        insert_count,
    );

    PollResult::NewData {
        expires_at: post_cache.expires_at,
        fetched_at: post_cache.fetched_at,
        model_run_at,
        extraction_count: extraction_result
            .forecasts
            .iter()
            .filter(|f| f.is_some())
            .count(),
    }
}

/// Extract the model run timestamp from a yr.no raw JSON response.
fn extract_model_run_at(raw_json: &serde_json::Value) -> Option<DateTime<Utc>> {
    raw_json
        .get("properties")?
        .get("meta")?
        .get("updated_at")?
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

/// Async sleep helper.
async fn sleep_secs(secs: u64) {
    tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floor_to_hour() {
        let dt = "2026-03-01T07:45:30Z".parse::<DateTime<Utc>>().unwrap();
        let floored = floor_to_hour(dt);
        assert_eq!(
            floored,
            "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn test_floor_to_hour_exact() {
        let dt = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let floored = floor_to_hour(dt);
        assert_eq!(floored, dt);
    }

    #[test]
    fn test_ceil_to_hour() {
        let dt = "2026-03-01T07:00:01Z".parse::<DateTime<Utc>>().unwrap();
        let ceiled = ceil_to_hour(dt);
        assert_eq!(
            ceiled,
            "2026-03-01T08:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn test_ceil_to_hour_exact() {
        let dt = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let ceiled = ceil_to_hour(dt);
        assert_eq!(ceiled, dt, "Exact hour should not be rounded up");
    }

    #[test]
    fn test_compute_extraction_times_start_checkpoint() {
        let race_start = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let times = compute_extraction_times(race_start, 0.0);
        assert_eq!(times.len(), 1);
        assert_eq!(times[0], race_start);
    }

    #[test]
    fn test_compute_extraction_times_mid_race() {
        // 45 km checkpoint:
        // earliest = 45/30 = 1.5 hours → 08:30 → floor to 08:00
        // latest   = 45/10 = 4.5 hours → 11:30 → ceil to 12:00
        // Expect: 08:00, 09:00, 10:00, 11:00, 12:00 = 5 slots
        let race_start = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let times = compute_extraction_times(race_start, 45.0);
        assert_eq!(times.len(), 5, "Expected 5 hourly slots, got {:?}", times);
        assert_eq!(
            times[0],
            "2026-03-01T08:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            times[4],
            "2026-03-01T12:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn test_compute_extraction_times_finish_90km() {
        // 90 km finish:
        // earliest = 90/30 = 3.0 hours → 10:00 (exact)
        // latest   = 90/10 = 9.0 hours → 16:00 (exact)
        // Expect: 10:00 through 16:00 = 7 slots
        let race_start = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let times = compute_extraction_times(race_start, 90.0);
        assert_eq!(times.len(), 7, "Expected 7 hourly slots, got {:?}", times);
        assert_eq!(
            times[0],
            "2026-03-01T10:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            times[6],
            "2026-03-01T16:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn test_compute_extraction_times_short_distance() {
        // 5 km checkpoint:
        // earliest = 5/30 = 0.167 hours = 10 min → 07:10 → floor to 07:00
        // latest   = 5/10 = 0.5 hours = 30 min → 07:30 → ceil to 08:00
        // Expect: 07:00, 08:00 = 2 slots
        let race_start = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let times = compute_extraction_times(race_start, 5.0);
        assert_eq!(times.len(), 2, "Expected 2 hourly slots, got {:?}", times);
        assert_eq!(
            times[0],
            "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            times[1],
            "2026-03-01T08:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn test_compute_extraction_times_monotonically_increasing() {
        let race_start = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let times = compute_extraction_times(race_start, 60.0);
        for i in 1..times.len() {
            assert!(
                times[i] > times[i - 1],
                "Times should be strictly increasing: {} vs {}",
                times[i - 1],
                times[i]
            );
        }
    }

    #[test]
    fn test_compute_extraction_times_all_on_hour_boundary() {
        let race_start = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let times = compute_extraction_times(race_start, 45.0);
        for t in &times {
            assert_eq!(
                t.time().minute(),
                0,
                "All times should be on the hour: {}",
                t
            );
            assert_eq!(
                t.time().second(),
                0,
                "All times should be on the hour: {}",
                t
            );
        }
    }

    #[test]
    fn test_extract_model_run_at_present() {
        let json = serde_json::json!({
            "properties": {
                "meta": {
                    "updated_at": "2026-02-28T14:00:00Z"
                },
                "timeseries": []
            }
        });
        let result = extract_model_run_at(&json);
        assert_eq!(
            result,
            Some("2026-02-28T14:00:00Z".parse::<DateTime<Utc>>().unwrap())
        );
    }

    #[test]
    fn test_extract_model_run_at_missing() {
        let json = serde_json::json!({
            "properties": {
                "timeseries": []
            }
        });
        let result = extract_model_run_at(&json);
        assert_eq!(result, None);
    }
}
