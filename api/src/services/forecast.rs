//! Forecast resolution service — extract-on-read architecture.
//!
//! Ensures the yr.no cache (yr_responses table) is fresh, then extracts
//! forecast data in-memory from the cached JSON. This avoids the bug where
//! a valid yr.no cache didn't have extracted forecasts for new checkpoints.
//!
//! Flow: request → ensure yr.no cache fresh → extract from cached JSON
//!       in-memory → respond (+ write to forecasts table for history).
//!
//! yr_responses is keyed by checkpoint_id (FK to checkpoints), with one
//! cache row per checkpoint. yr.no's Expires header controls freshness,
//! If-Modified-Since enables conditional requests.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

use crate::db::models::{Checkpoint, Forecast};
use crate::db::queries::{self, InsertForecastParams};
use crate::errors::AppError;
use crate::services::yr::{
    extract_forecasts_at_times, parse_expires_header, YrClient, YrParsedForecast,
    YrTimeseriesResult,
};

/// Calculate the "feels like" temperature using the North American Wind Chill Index.
///
/// Formula: 13.12 + 0.6215*T - 11.37*V^0.16 + 0.3965*T*V^0.16
/// Applied when T <= 10°C and V >= 4.8 km/h.
///
/// T: temperature in Celsius
/// V: wind speed in km/h
pub fn calculate_feels_like(temperature_c: f64, wind_speed_ms: f64) -> f64 {
    let wind_speed_kmh = wind_speed_ms * 3.6;

    if temperature_c > 10.0 || wind_speed_kmh < 4.8 {
        return temperature_c;
    }

    let v016 = wind_speed_kmh.powf(0.16);
    13.12 + 0.6215 * temperature_c - 11.37 * v016 + 0.3965 * temperature_c * v016
}

/// Infer precipitation type from yr.no symbol_code and temperature.
///
/// Primary: parse from symbol_code string (contains "snow", "rain", "sleet").
/// Fallback: temperature-based heuristic.
pub fn infer_precipitation_type(
    symbol_code: &str,
    temperature_c: f64,
    precipitation_mm: f64,
) -> String {
    if precipitation_mm <= 0.0 {
        return "none".to_string();
    }

    let code_lower = symbol_code.to_lowercase();

    // Check symbol_code first
    if code_lower.contains("snow") {
        return "snow".to_string();
    }
    if code_lower.contains("sleet") {
        return "sleet".to_string();
    }
    if code_lower.contains("rain") || code_lower.contains("drizzle") {
        return "rain".to_string();
    }

    // Temperature-based fallback
    if temperature_c < 0.0 {
        "snow".to_string()
    } else if temperature_c <= 2.0 {
        "sleet".to_string()
    } else {
        "rain".to_string()
    }
}

/// Calculate the expected pass-through time for a checkpoint using even pacing.
///
/// pass_time = start_time + duration * (checkpoint.distance_km / race.distance_km)
///
/// Superseded by `calculate_pass_time_weighted` + `calculate_pass_time_fractions`
/// for elevation-adjusted pacing. Retained for tests.
#[cfg(test)]
fn calculate_pass_time(
    start_time: DateTime<Utc>,
    checkpoint_distance_km: f64,
    race_distance_km: f64,
    target_duration_hours: f64,
) -> DateTime<Utc> {
    let fraction = checkpoint_distance_km / race_distance_km;
    let duration_secs = (target_duration_hours * 3600.0 * fraction) as i64;
    start_time + Duration::seconds(duration_secs)
}

// --- Elevation-adjusted pacing ---
//
// Distributes total race time across segments proportionally to effort cost,
// which accounts for gradient. Uphill segments get more time, downhill less,
// while the total duration stays exactly the same as the user's target.

/// Uphill cost multiplier per unit gradient (m/m).
/// A 5% uphill grade → cost factor 1.6× per km.
const K_UP: f64 = 12.0;

/// Downhill cost multiplier per unit gradient (m/m).
/// A 5% downhill grade → cost factor 0.8× per km.
const K_DOWN: f64 = 4.0;

/// Minimum cost factor per km (floor). Even steep downhill isn't free in XC skiing.
const MIN_COST_FACTOR: f64 = 0.5;

/// Input for elevation-adjusted pacing calculation.
pub struct PacingCheckpoint {
    pub distance_km: f64,
    pub elevation_m: f64,
}

/// Compute cumulative time fractions for each checkpoint based on elevation profile.
///
/// Returns a `Vec<f64>` of the same length as `checkpoints`, where:
/// - index 0 is always 0.0 (start)
/// - last index is always 1.0 (finish)
/// - intermediate values reflect effort-weighted cumulative time
///
/// If there are fewer than 2 checkpoints, returns trivial fractions.
/// Falls back to even (distance-based) pacing if total distance is zero.
pub fn calculate_pass_time_fractions(checkpoints: &[PacingCheckpoint]) -> Vec<f64> {
    let n = checkpoints.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![0.0];
    }

    // Compute cost for each segment between consecutive checkpoints
    let mut segment_costs = Vec::with_capacity(n - 1);
    for i in 0..(n - 1) {
        let dist_delta = checkpoints[i + 1].distance_km - checkpoints[i].distance_km;
        if dist_delta <= 0.0 {
            // Zero-length or negative segment — assign minimal cost
            segment_costs.push(0.0);
            continue;
        }

        let ele_delta = checkpoints[i + 1].elevation_m - checkpoints[i].elevation_m;
        // gradient in m/m (rise over run)
        let gradient = ele_delta / (dist_delta * 1000.0);

        let cost_factor = if gradient >= 0.0 {
            // Uphill: penalise
            (1.0 + K_UP * gradient).max(MIN_COST_FACTOR)
        } else {
            // Downhill: bonus (gradient is negative, K_DOWN is positive)
            (1.0 - K_DOWN * gradient.abs()).max(MIN_COST_FACTOR)
        };

        segment_costs.push(cost_factor * dist_delta);
    }

    let total_cost: f64 = segment_costs.iter().sum();
    if total_cost <= 0.0 {
        // Degenerate case — fall back to even pacing by distance
        let total_dist = checkpoints.last().unwrap().distance_km;
        if total_dist <= 0.0 {
            return (0..n).map(|i| i as f64 / (n - 1) as f64).collect();
        }
        return checkpoints
            .iter()
            .map(|cp| cp.distance_km / total_dist)
            .collect();
    }

    // Build cumulative fractions
    let mut fractions = Vec::with_capacity(n);
    fractions.push(0.0);
    let mut cumulative = 0.0;
    for cost in &segment_costs {
        cumulative += cost;
        fractions.push(cumulative / total_cost);
    }

    // Ensure last fraction is exactly 1.0 (avoid floating-point drift)
    if let Some(last) = fractions.last_mut() {
        *last = 1.0;
    }

    fractions
}

/// Calculate expected pass-through time from a precomputed time fraction.
///
/// pass_time = start_time + target_duration * fraction
pub fn calculate_pass_time_weighted(
    start_time: DateTime<Utc>,
    time_fraction: f64,
    target_duration_hours: f64,
) -> DateTime<Utc> {
    let duration_secs = (target_duration_hours * 3600.0 * time_fraction) as i64;
    start_time + Duration::seconds(duration_secs)
}

/// Ensure the yr.no cache is fresh for a given checkpoint. Does NOT extract forecasts.
///
/// Returns the cached raw_response JSON (either still-valid cache or just-fetched).
/// Callers extract forecast data in-memory from the returned JSON (extract-on-read).
///
/// This fixes the cache-valid-but-no-extracted-forecast bug: previously, when the
/// cache was still valid, the old function returned immediately without extracting
/// forecasts for new checkpoints at already-cached locations.
async fn ensure_yr_cache_fresh(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoint: &Checkpoint,
) -> Result<serde_json::Value, AppError> {
    let checkpoint_id = checkpoint.id;

    // 1. Check for a non-expired cached response
    if let Some(cached) = queries::get_yr_cached_response(pool, checkpoint_id).await? {
        return Ok(cached.raw_response);
    }

    // 2. Cache miss or expired — try conditional request with If-Modified-Since
    let existing = queries::get_yr_cached_response_any(pool, checkpoint_id).await?;
    let if_modified_since = existing.as_ref().and_then(|c| c.last_modified.as_deref());

    let lat = checkpoint.latitude.to_f64().unwrap_or(0.0);
    let lon = checkpoint.longitude.to_f64().unwrap_or(0.0);
    let alt = checkpoint.elevation_m.to_f64().unwrap_or(0.0);

    match yr_client
        .fetch_timeseries(lat, lon, alt, if_modified_since)
        .await?
    {
        YrTimeseriesResult::NewData {
            raw_json,
            expires,
            last_modified,
        } => {
            let expires_at = expires
                .as_deref()
                .map(parse_expires_header)
                .unwrap_or_else(|| Utc::now() + Duration::hours(1));

            queries::upsert_yr_cached_response(
                pool,
                checkpoint_id,
                checkpoint.latitude,
                checkpoint.longitude,
                checkpoint.elevation_m,
                Utc::now(),
                expires_at,
                last_modified.as_deref(),
                &raw_json,
            )
            .await?;

            Ok(raw_json)
        }
        YrTimeseriesResult::NotModified => {
            if let Some(cached) = existing {
                // Just bump expires_at — don't re-write the full JSON blob
                let new_expires = Utc::now() + Duration::hours(1);
                queries::update_yr_cache_expiry(pool, checkpoint_id, new_expires).await?;
                Ok(cached.raw_response)
            } else {
                Err(AppError::ExternalServiceError(
                    "yr.no returned 304 but no cached data exists".to_string(),
                ))
            }
        }
    }
}

/// Build `InsertForecastParams` for a single parsed yr.no entry for a checkpoint.
fn build_single_insert_params(
    checkpoint_id: Uuid,
    parsed: &YrParsedForecast,
    fetched_at: DateTime<Utc>,
) -> InsertForecastParams {
    let temp_c = parsed.temperature_c.to_f64().unwrap_or(0.0);
    let wind_ms = parsed.wind_speed_ms.to_f64().unwrap_or(0.0);
    let precip_mm = parsed.precipitation_mm.to_f64().unwrap_or(0.0);

    let feels_like = calculate_feels_like(temp_c, wind_ms);
    let precip_type = infer_precipitation_type(&parsed.symbol_code, temp_c, precip_mm);
    let feels_like_dec = Decimal::from_str(&format!("{:.1}", feels_like)).unwrap_or_default();

    InsertForecastParams {
        checkpoint_id,
        forecast_time: parsed.forecast_time,
        fetched_at,
        source: "yr.no".to_string(),
        temperature_c: parsed.temperature_c,
        temperature_percentile_10_c: parsed.temperature_percentile_10_c,
        temperature_percentile_90_c: parsed.temperature_percentile_90_c,
        wind_speed_ms: parsed.wind_speed_ms,
        wind_speed_percentile_10_ms: parsed.wind_speed_percentile_10_ms,
        wind_speed_percentile_90_ms: parsed.wind_speed_percentile_90_ms,
        wind_direction_deg: parsed.wind_direction_deg,
        wind_gust_ms: parsed.wind_gust_ms,
        precipitation_mm: parsed.precipitation_mm,
        precipitation_min_mm: parsed.precipitation_min_mm,
        precipitation_max_mm: parsed.precipitation_max_mm,
        humidity_pct: parsed.humidity_pct,
        dew_point_c: parsed.dew_point_c,
        cloud_cover_pct: parsed.cloud_cover_pct,
        uv_index: parsed.uv_index,
        symbol_code: parsed.symbol_code.clone(),
        feels_like_c: feels_like_dec,
        precipitation_type: precip_type,
        yr_model_run_at: parsed.yr_model_run_at,
    }
}

/// Resolve the forecast for a single checkpoint (extract-on-read).
///
/// 1. Ensures the yr.no cache is fresh for the checkpoint's location.
/// 2. Extracts the forecast from the cached JSON in-memory.
/// 3. Writes to the forecasts table for history (ON CONFLICT DO NOTHING).
/// 4. Re-queries the DB for the canonical forecast row.
///
/// Returns `(Some(forecast), is_stale)` when a forecast is available, or
/// `(None, false)` when yr.no doesn't cover the requested time (e.g. race
/// date is beyond the ~10-day forecast horizon).
pub async fn resolve_forecast(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoint: &Checkpoint,
    forecast_time: DateTime<Utc>,
) -> Result<(Option<Forecast>, bool), AppError> {
    // Step 1: Try to get fresh yr.no data
    let raw_json = match ensure_yr_cache_fresh(pool, yr_client, checkpoint).await {
        Ok(json) => json,
        Err(e) => {
            // yr.no failed — fall back to cached forecast from DB
            let cached = queries::get_latest_forecast(pool, checkpoint.id, forecast_time).await?;
            if let Some(forecast) = cached {
                tracing::warn!("yr.no unavailable, returning stale data: {}", e);
                return Ok((Some(forecast), true));
            }
            return Err(AppError::ExternalServiceError(format!(
                "yr.no unavailable and no cached data: {}",
                e
            )));
        }
    };

    // Step 2: Extract forecast from cached JSON in-memory (extract-on-read)
    let parsed = extract_forecasts_at_times(raw_json, &[forecast_time])?;
    let maybe_parsed = parsed.into_iter().next().flatten();

    match maybe_parsed {
        Some(ref forecast_data) => {
            // Step 3: Write to forecasts table for history (ON CONFLICT DO NOTHING)
            let params = build_single_insert_params(checkpoint.id, forecast_data, Utc::now());
            let _ = queries::insert_forecast(pool, params).await?;

            // Step 4: Re-query DB for the canonical forecast row
            let forecast = queries::get_latest_forecast(pool, checkpoint.id, forecast_time).await?;
            Ok((forecast, false))
        }
        None => {
            // Beyond yr.no horizon — no forecast available for this time
            Ok((None, false))
        }
    }
}

/// Checkpoint with its expected pass-through time (for batch resolution).
pub struct CheckpointWithTime {
    pub checkpoint: Checkpoint,
    pub forecast_time: DateTime<Utc>,
}

/// Result of resolving a forecast for a checkpoint.
#[derive(Clone)]
pub struct ResolvedForecast {
    /// The forecast data, or `None` if yr.no doesn't cover the requested time
    /// (e.g. race date is beyond the ~10-day forecast horizon).
    pub forecast: Option<Forecast>,
    /// Whether this result is served from stale cache (yr.no was unreachable).
    pub is_stale: bool,
}

/// Resolve forecasts for multiple checkpoints in a race — extract-on-read.
///
/// 1. `ensure_yr_cache_fresh` for each checkpoint (parallel)
/// 2. Extract forecasts from cached JSON in-memory for all checkpoints
/// 3. Write to forecasts table for history (ON CONFLICT DO NOTHING)
/// 4. Re-query DB for canonical Forecast rows (batch)
///
/// Each checkpoint has its own yr_responses row (keyed by checkpoint_id FK),
/// so there is no location-based grouping.
pub async fn resolve_race_forecasts(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoints: &[CheckpointWithTime],
) -> Result<Vec<ResolvedForecast>, AppError> {
    let n = checkpoints.len();

    // ── Step 1: Ensure yr.no cache fresh for each checkpoint (bounded parallel) ──
    // Limit concurrency to avoid overwhelming yr.no with simultaneous requests.
    use futures::stream::{self, StreamExt};
    const MAX_CONCURRENT_YR_FETCHES: usize = 4;

    let futures: Vec<_> = checkpoints
        .iter()
        .map(|cpwt| {
            let pool = pool.clone();
            let yr_client = yr_client.clone();
            let checkpoint = cpwt.checkpoint.clone();
            async move { ensure_yr_cache_fresh(&pool, &yr_client, &checkpoint).await }
        })
        .collect();

    let fetch_results: Vec<Result<serde_json::Value, AppError>> = stream::iter(futures)
        .buffer_unordered(MAX_CONCURRENT_YR_FETCHES)
        .collect()
        .await;

    // ── Step 2: Handle results, falling back to DB cache on error ──
    // Pre-fetch cached forecasts for fallback (batch query)
    let pairs: Vec<(Uuid, DateTime<Utc>)> = checkpoints
        .iter()
        .map(|cpwt| (cpwt.checkpoint.id, cpwt.forecast_time))
        .collect();
    let cached_forecasts = queries::get_latest_forecasts_batch(pool, &pairs).await?;

    let mut results: Vec<Option<ResolvedForecast>> = vec![None; n];

    for (idx, fetch_result) in fetch_results.into_iter().enumerate() {
        match fetch_result {
            Ok(raw_json) => {
                // Extract forecast from cached JSON in-memory
                let forecast_time = checkpoints[idx].forecast_time;
                let parsed = extract_forecasts_at_times(raw_json, &[forecast_time])?;
                let maybe_parsed = parsed.into_iter().next().flatten();

                match maybe_parsed {
                    Some(ref forecast_data) => {
                        // Write to forecasts table for history (ON CONFLICT DO NOTHING)
                        let params = build_single_insert_params(
                            checkpoints[idx].checkpoint.id,
                            forecast_data,
                            Utc::now(),
                        );
                        let _ = queries::insert_forecast(pool, params).await?;

                        // Mark for batch re-query below
                        results[idx] = None; // will be filled by batch re-query
                    }
                    None => {
                        // Beyond yr.no horizon — no forecast available
                        results[idx] = Some(ResolvedForecast {
                            forecast: None,
                            is_stale: false,
                        });
                    }
                }
            }
            Err(e) => {
                // yr.no failed for this checkpoint — fall back to cached forecast
                if let Some(cached) = cached_forecasts[idx].clone() {
                    tracing::warn!(
                        "yr.no unavailable for checkpoint {}, will use stale DB data: {}",
                        checkpoints[idx].checkpoint.id,
                        e
                    );
                    results[idx] = Some(ResolvedForecast {
                        forecast: Some(cached),
                        is_stale: true,
                    });
                } else {
                    return Err(AppError::ExternalServiceError(format!(
                        "yr.no unavailable for checkpoint {} and no cached data: {}",
                        checkpoints[idx].checkpoint.id, e
                    )));
                }
            }
        }
    }

    // ── Step 3: Batch re-query DB for canonical Forecast rows ──
    // Collect indices that need re-query (successfully extracted, not stale fallback)
    let requery_pairs: Vec<(Uuid, DateTime<Utc>)> = results
        .iter()
        .enumerate()
        .filter(|(_, r)| r.is_none())
        .map(|(idx, _)| {
            (
                checkpoints[idx].checkpoint.id,
                checkpoints[idx].forecast_time,
            )
        })
        .collect();

    let requeried = queries::get_latest_forecasts_batch(pool, &requery_pairs).await?;

    let mut requery_iter = requeried.into_iter();
    for result in results.iter_mut() {
        if result.is_none() {
            *result = Some(ResolvedForecast {
                forecast: requery_iter.next().unwrap_or(None),
                is_stale: false,
            });
        }
    }

    results
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            r.ok_or_else(|| {
                AppError::InternalError(format!(
                    "Missing resolved forecast for checkpoint index {}",
                    i
                ))
            })
        })
        .collect()
}

/// Resolve a checkpoint by ID from the database.
pub async fn get_checkpoint(pool: &PgPool, checkpoint_id: Uuid) -> Result<Checkpoint, AppError> {
    queries::get_checkpoint(pool, checkpoint_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Checkpoint {} not found", checkpoint_id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feels_like_cold_and_windy() {
        // -4°C with 3.2 m/s wind -> should apply wind chill
        let result = calculate_feels_like(-4.0, 3.2);
        // Wind at 3.2 m/s = 11.52 km/h (> 4.8)
        assert!(result < -4.0, "Feels like should be colder: {}", result);
    }

    #[test]
    fn test_feels_like_warm() {
        // 15°C — above 10°C threshold, returns temperature as-is
        let result = calculate_feels_like(15.0, 5.0);
        assert_eq!(result, 15.0);
    }

    #[test]
    fn test_feels_like_no_wind() {
        // -5°C but very low wind -> returns temperature
        let result = calculate_feels_like(-5.0, 1.0); // 3.6 km/h < 4.8
        assert_eq!(result, -5.0);
    }

    #[test]
    fn test_feels_like_zero_wind() {
        let result = calculate_feels_like(-10.0, 0.0);
        assert_eq!(result, -10.0);
    }

    #[test]
    fn test_precip_type_from_symbol_snow() {
        assert_eq!(infer_precipitation_type("heavysnow", -5.0, 2.0), "snow");
    }

    #[test]
    fn test_precip_type_from_symbol_rain() {
        assert_eq!(infer_precipitation_type("lightrain", 5.0, 1.0), "rain");
    }

    #[test]
    fn test_precip_type_from_symbol_sleet() {
        assert_eq!(infer_precipitation_type("sleet", 1.0, 0.5), "sleet");
    }

    #[test]
    fn test_precip_type_none_when_no_precipitation() {
        assert_eq!(infer_precipitation_type("clearsky_day", -5.0, 0.0), "none");
    }

    #[test]
    fn test_precip_type_fallback_cold() {
        assert_eq!(infer_precipitation_type("cloudy", -3.0, 1.0), "snow");
    }

    #[test]
    fn test_precip_type_fallback_warm() {
        assert_eq!(infer_precipitation_type("cloudy", 5.0, 1.0), "rain");
    }

    #[test]
    fn test_precip_type_fallback_borderline() {
        assert_eq!(infer_precipitation_type("cloudy", 1.0, 1.0), "sleet");
    }

    #[test]
    fn test_pacing_start() {
        let start = DateTime::parse_from_rfc3339("2026-03-01T07:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let result = calculate_pass_time(start, 0.0, 90.0, 8.0);
        assert_eq!(result, start);
    }

    #[test]
    fn test_pacing_finish() {
        let start = DateTime::parse_from_rfc3339("2026-03-01T07:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let result = calculate_pass_time(start, 90.0, 90.0, 8.0);
        let expected = start + Duration::hours(8);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_pacing_midpoint() {
        let start = DateTime::parse_from_rfc3339("2026-03-01T07:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let result = calculate_pass_time(start, 45.0, 90.0, 8.0);
        let expected = start + Duration::hours(4);
        assert_eq!(result, expected);
    }

    // --- Elevation-adjusted pacing tests ---

    #[test]
    fn test_elevation_fractions_flat_course() {
        // All same elevation → should produce same fractions as even pacing
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 100.0,
            },
            PacingCheckpoint {
                distance_km: 30.0,
                elevation_m: 100.0,
            },
            PacingCheckpoint {
                distance_km: 60.0,
                elevation_m: 100.0,
            },
            PacingCheckpoint {
                distance_km: 90.0,
                elevation_m: 100.0,
            },
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 4);
        assert!((fractions[0] - 0.0).abs() < 1e-10);
        assert!((fractions[1] - 1.0 / 3.0).abs() < 1e-10);
        assert!((fractions[2] - 2.0 / 3.0).abs() < 1e-10);
        assert!((fractions[3] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_elevation_fractions_uphill_gets_more_time() {
        // Uphill first half, flat second half
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 0.0,
            },
            PacingCheckpoint {
                distance_km: 45.0,
                elevation_m: 500.0,
            }, // +500m over 45km
            PacingCheckpoint {
                distance_km: 90.0,
                elevation_m: 500.0,
            }, // flat
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 3);
        assert!((fractions[0] - 0.0).abs() < 1e-10);
        // Midpoint should be > 0.5 (uphill first half takes more time)
        assert!(
            fractions[1] > 0.5,
            "Uphill half should take more than 50% of time, got {}",
            fractions[1]
        );
        assert!((fractions[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_elevation_fractions_downhill_gets_less_time() {
        // Flat first half, downhill second half
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 500.0,
            },
            PacingCheckpoint {
                distance_km: 45.0,
                elevation_m: 500.0,
            }, // flat
            PacingCheckpoint {
                distance_km: 90.0,
                elevation_m: 0.0,
            }, // -500m over 45km
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 3);
        // Midpoint should be > 0.5 (downhill second half takes less time,
        // so more of the time is spent in the flat first half)
        assert!(
            fractions[1] > 0.5,
            "Flat half before downhill should take more than 50% of time, got {}",
            fractions[1]
        );
        assert!((fractions[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_elevation_fractions_total_is_one() {
        // Vasaloppet-like profile
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 349.0,
            },
            PacingCheckpoint {
                distance_km: 11.0,
                elevation_m: 502.0,
            },
            PacingCheckpoint {
                distance_km: 24.0,
                elevation_m: 390.0,
            },
            PacingCheckpoint {
                distance_km: 35.0,
                elevation_m: 396.0,
            },
            PacingCheckpoint {
                distance_km: 47.0,
                elevation_m: 419.0,
            },
            PacingCheckpoint {
                distance_km: 62.0,
                elevation_m: 231.0,
            },
            PacingCheckpoint {
                distance_km: 71.0,
                elevation_m: 247.0,
            },
            PacingCheckpoint {
                distance_km: 81.0,
                elevation_m: 206.0,
            },
            PacingCheckpoint {
                distance_km: 90.0,
                elevation_m: 168.0,
            },
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 9);
        assert!((fractions[0] - 0.0).abs() < 1e-10, "Start should be 0.0");
        assert!((fractions[8] - 1.0).abs() < 1e-10, "Finish should be 1.0");

        // All fractions should be monotonically increasing
        for i in 1..fractions.len() {
            assert!(
                fractions[i] >= fractions[i - 1],
                "Fractions should be monotonically increasing: f[{}]={} < f[{}]={}",
                i,
                fractions[i],
                i - 1,
                fractions[i - 1]
            );
        }
    }

    #[test]
    fn test_elevation_fractions_vasaloppet_first_segment_slower() {
        // Berga→Smågan is the steepest uphill (+153m over 11km)
        // It should take more than its distance fraction (11/90 ≈ 0.122)
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 349.0,
            },
            PacingCheckpoint {
                distance_km: 11.0,
                elevation_m: 502.0,
            },
            PacingCheckpoint {
                distance_km: 24.0,
                elevation_m: 390.0,
            },
            PacingCheckpoint {
                distance_km: 35.0,
                elevation_m: 396.0,
            },
            PacingCheckpoint {
                distance_km: 47.0,
                elevation_m: 419.0,
            },
            PacingCheckpoint {
                distance_km: 62.0,
                elevation_m: 231.0,
            },
            PacingCheckpoint {
                distance_km: 71.0,
                elevation_m: 247.0,
            },
            PacingCheckpoint {
                distance_km: 81.0,
                elevation_m: 206.0,
            },
            PacingCheckpoint {
                distance_km: 90.0,
                elevation_m: 168.0,
            },
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        let even_fraction = 11.0 / 90.0;
        assert!(
            fractions[1] > even_fraction,
            "Berga→Smågan should take more than even pacing ({:.3}), got {:.3}",
            even_fraction,
            fractions[1]
        );
    }

    #[test]
    fn test_elevation_weighted_pass_time() {
        let start = DateTime::parse_from_rfc3339("2026-03-01T07:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        // fraction=0.0 → start, fraction=1.0 → start + 8h
        assert_eq!(calculate_pass_time_weighted(start, 0.0, 8.0), start);
        assert_eq!(
            calculate_pass_time_weighted(start, 1.0, 8.0),
            start + Duration::hours(8)
        );
        // fraction=0.25 → start + 2h
        assert_eq!(
            calculate_pass_time_weighted(start, 0.25, 8.0),
            start + Duration::hours(2)
        );
    }

    #[test]
    fn test_elevation_fractions_empty() {
        let fractions = calculate_pass_time_fractions(&[]);
        assert!(fractions.is_empty());
    }

    #[test]
    fn test_elevation_fractions_single() {
        let fractions = calculate_pass_time_fractions(&[PacingCheckpoint {
            distance_km: 0.0,
            elevation_m: 100.0,
        }]);
        assert_eq!(fractions.len(), 1);
        assert!((fractions[0] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_build_single_insert_params() {
        use crate::services::yr::{ForecastResolution, YrParsedForecast};

        let checkpoint_id = Uuid::new_v4();
        let fetched_at = Utc::now();
        let model_run = "2026-02-28T14:00:00Z".parse::<DateTime<Utc>>().unwrap();

        let forecast = YrParsedForecast {
            forecast_time: "2026-03-01T07:00:00Z".parse().unwrap(),
            temperature_c: Decimal::from_str("-5.0").unwrap(),
            temperature_percentile_10_c: None,
            temperature_percentile_90_c: None,
            wind_speed_ms: Decimal::from_str("3.2").unwrap(),
            wind_speed_percentile_10_ms: None,
            wind_speed_percentile_90_ms: None,
            wind_direction_deg: Decimal::from_str("180.0").unwrap(),
            wind_gust_ms: None,
            precipitation_mm: Decimal::from_str("0.5").unwrap(),
            precipitation_min_mm: None,
            precipitation_max_mm: None,
            humidity_pct: Decimal::from_str("75.0").unwrap(),
            dew_point_c: Decimal::from_str("-8.5").unwrap(),
            cloud_cover_pct: Decimal::from_str("50.0").unwrap(),
            uv_index: None,
            symbol_code: "lightsnow".to_string(),
            yr_model_run_at: Some(model_run),
            resolution: ForecastResolution::Hourly,
        };

        let params = build_single_insert_params(checkpoint_id, &forecast, fetched_at);

        // yr.no native time preserved
        assert_eq!(
            params.forecast_time,
            "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(params.checkpoint_id, checkpoint_id);
        assert_eq!(params.source, "yr.no");
        assert_eq!(params.yr_model_run_at, Some(model_run));

        // Feels-like should be computed (cold + wind -> colder)
        let feels_like_f64 = params.feels_like_c.to_f64().unwrap();
        assert!(feels_like_f64 < -5.0, "Wind chill should lower feels_like");

        // Precipitation type: symbol_code "lightsnow" → "snow"
        assert_eq!(params.precipitation_type, "snow");
    }

    #[test]
    fn test_build_single_insert_params_all_optional_fields_none() {
        use crate::services::yr::{ForecastResolution, YrParsedForecast};

        let checkpoint_id = Uuid::new_v4();
        let fetched_at = Utc::now();

        let forecast = YrParsedForecast {
            forecast_time: "2026-03-01T07:00:00Z".parse().unwrap(),
            temperature_c: Decimal::from_str("2.0").unwrap(),
            temperature_percentile_10_c: None,
            temperature_percentile_90_c: None,
            wind_speed_ms: Decimal::from_str("0.5").unwrap(),
            wind_speed_percentile_10_ms: None,
            wind_speed_percentile_90_ms: None,
            wind_direction_deg: Decimal::from_str("90.0").unwrap(),
            wind_gust_ms: None,
            precipitation_mm: Decimal::from_str("0.0").unwrap(),
            precipitation_min_mm: None,
            precipitation_max_mm: None,
            humidity_pct: Decimal::from_str("60.0").unwrap(),
            dew_point_c: Decimal::from_str("-2.0").unwrap(),
            cloud_cover_pct: Decimal::from_str("0.0").unwrap(),
            uv_index: None,
            symbol_code: "clearsky_day".to_string(),
            yr_model_run_at: None,
            resolution: ForecastResolution::SixHourly,
        };

        let params = build_single_insert_params(checkpoint_id, &forecast, fetched_at);

        // All optional fields should be None
        assert!(params.temperature_percentile_10_c.is_none());
        assert!(params.temperature_percentile_90_c.is_none());
        assert!(params.wind_speed_percentile_10_ms.is_none());
        assert!(params.wind_speed_percentile_90_ms.is_none());
        assert!(params.wind_gust_ms.is_none());
        assert!(params.precipitation_min_mm.is_none());
        assert!(params.precipitation_max_mm.is_none());
        assert!(params.uv_index.is_none());
        assert!(params.yr_model_run_at.is_none());

        // Zero precip -> "none"
        assert_eq!(params.precipitation_type, "none");

        // Warm temp (2°C) with very low wind (0.5 m/s = 1.8 km/h < 4.8)
        // -> no wind chill applied, feels_like equals temperature
        let feels_like_f64 = params.feels_like_c.to_f64().unwrap();
        assert!(
            (feels_like_f64 - 2.0).abs() < 0.1,
            "Warm + no wind: feels_like should equal temperature, got {}",
            feels_like_f64
        );
    }

    #[test]
    fn test_build_single_insert_params_all_optional_fields_some() {
        use crate::services::yr::{ForecastResolution, YrParsedForecast};

        let checkpoint_id = Uuid::new_v4();
        let fetched_at = Utc::now();
        let model_run = "2026-02-28T06:00:00Z".parse::<DateTime<Utc>>().unwrap();

        let forecast = YrParsedForecast {
            forecast_time: "2026-03-01T10:00:00Z".parse().unwrap(),
            temperature_c: Decimal::from_str("-8.0").unwrap(),
            temperature_percentile_10_c: Some(Decimal::from_str("-10.0").unwrap()),
            temperature_percentile_90_c: Some(Decimal::from_str("-6.0").unwrap()),
            wind_speed_ms: Decimal::from_str("5.0").unwrap(),
            wind_speed_percentile_10_ms: Some(Decimal::from_str("3.0").unwrap()),
            wind_speed_percentile_90_ms: Some(Decimal::from_str("8.0").unwrap()),
            wind_direction_deg: Decimal::from_str("270.0").unwrap(),
            wind_gust_ms: Some(Decimal::from_str("12.0").unwrap()),
            precipitation_mm: Decimal::from_str("1.5").unwrap(),
            precipitation_min_mm: Some(Decimal::from_str("0.5").unwrap()),
            precipitation_max_mm: Some(Decimal::from_str("3.0").unwrap()),
            humidity_pct: Decimal::from_str("90.0").unwrap(),
            dew_point_c: Decimal::from_str("-9.5").unwrap(),
            cloud_cover_pct: Decimal::from_str("100.0").unwrap(),
            uv_index: Some(Decimal::from_str("0.5").unwrap()),
            symbol_code: "heavysnow".to_string(),
            yr_model_run_at: Some(model_run),
            resolution: ForecastResolution::Hourly,
        };

        let params = build_single_insert_params(checkpoint_id, &forecast, fetched_at);

        // All optional fields should be Some and pass through
        assert_eq!(
            params.temperature_percentile_10_c,
            Some(Decimal::from_str("-10.0").unwrap())
        );
        assert_eq!(
            params.temperature_percentile_90_c,
            Some(Decimal::from_str("-6.0").unwrap())
        );
        assert_eq!(
            params.wind_speed_percentile_10_ms,
            Some(Decimal::from_str("3.0").unwrap())
        );
        assert_eq!(
            params.wind_speed_percentile_90_ms,
            Some(Decimal::from_str("8.0").unwrap())
        );
        assert_eq!(
            params.wind_gust_ms,
            Some(Decimal::from_str("12.0").unwrap())
        );
        assert_eq!(
            params.precipitation_min_mm,
            Some(Decimal::from_str("0.5").unwrap())
        );
        assert_eq!(
            params.precipitation_max_mm,
            Some(Decimal::from_str("3.0").unwrap())
        );
        assert_eq!(params.uv_index, Some(Decimal::from_str("0.5").unwrap()));
        assert_eq!(params.yr_model_run_at, Some(model_run));

        // Cold + windy -> wind chill should lower it significantly
        let feels_like_f64 = params.feels_like_c.to_f64().unwrap();
        assert!(
            feels_like_f64 < -12.0,
            "-8°C + 5 m/s wind: feels_like should be well below -8, got {}",
            feels_like_f64
        );

        assert_eq!(params.precipitation_type, "snow");
    }

    #[test]
    fn test_build_single_insert_params_zero_precip_with_snow_symbol() {
        use crate::services::yr::{ForecastResolution, YrParsedForecast};

        // Edge case: symbol says "snow" but precipitation is 0.0 -> should be "none"
        let forecast = YrParsedForecast {
            forecast_time: "2026-03-01T07:00:00Z".parse().unwrap(),
            temperature_c: Decimal::from_str("-5.0").unwrap(),
            temperature_percentile_10_c: None,
            temperature_percentile_90_c: None,
            wind_speed_ms: Decimal::from_str("2.0").unwrap(),
            wind_speed_percentile_10_ms: None,
            wind_speed_percentile_90_ms: None,
            wind_direction_deg: Decimal::from_str("0.0").unwrap(),
            wind_gust_ms: None,
            precipitation_mm: Decimal::from_str("0.0").unwrap(),
            precipitation_min_mm: None,
            precipitation_max_mm: None,
            humidity_pct: Decimal::from_str("50.0").unwrap(),
            dew_point_c: Decimal::from_str("-10.0").unwrap(),
            cloud_cover_pct: Decimal::from_str("80.0").unwrap(),
            uv_index: None,
            symbol_code: "lightsnow".to_string(),
            yr_model_run_at: None,
            resolution: ForecastResolution::Hourly,
        };

        let params = build_single_insert_params(Uuid::new_v4(), &forecast, Utc::now());
        assert_eq!(params.precipitation_type, "none");
    }

    // --- calculate_pass_time_fractions edge cases ---

    #[test]
    fn test_elevation_fractions_two_checkpoints() {
        // Minimal non-trivial case: just start and finish
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 300.0,
            },
            PacingCheckpoint {
                distance_km: 90.0,
                elevation_m: 160.0,
            },
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 2);
        assert!((fractions[0] - 0.0).abs() < 1e-10);
        assert!((fractions[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_elevation_fractions_zero_length_segment() {
        // Two checkpoints at the same distance in the middle of a course
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 300.0,
            },
            PacingCheckpoint {
                distance_km: 45.0,
                elevation_m: 500.0,
            },
            PacingCheckpoint {
                distance_km: 45.0,
                elevation_m: 500.0,
            }, // duplicate distance
            PacingCheckpoint {
                distance_km: 90.0,
                elevation_m: 160.0,
            },
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 4);
        assert!((fractions[0] - 0.0).abs() < 1e-10);
        assert!((fractions[3] - 1.0).abs() < 1e-10);
        // The zero-length segment should have no cost, so fractions[1] == fractions[2]
        assert!(
            (fractions[1] - fractions[2]).abs() < 1e-10,
            "Zero-length segment should have equal fractions: {} vs {}",
            fractions[1],
            fractions[2]
        );
        // Fractions should be monotonically non-decreasing
        for i in 0..(fractions.len() - 1) {
            assert!(
                fractions[i] <= fractions[i + 1] + 1e-10,
                "Fractions should be monotonically non-decreasing"
            );
        }
    }

    #[test]
    fn test_elevation_fractions_all_distance_zero() {
        // All checkpoints at distance 0 — triggers degenerate fallback
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 100.0,
            },
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 200.0,
            },
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 300.0,
            },
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 3);
        // Falls back to evenly spaced: 0.0, 0.5, 1.0
        assert!((fractions[0] - 0.0).abs() < 1e-10);
        assert!((fractions[1] - 0.5).abs() < 1e-10);
        assert!((fractions[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_elevation_fractions_steep_downhill_hits_min_cost() {
        // Extremely steep downhill: 1000m drop over 1km -> gradient = -1.0
        // cost_factor = (1.0 - 4.0 * 1.0) = -3.0 -> clamped to MIN_COST_FACTOR (0.5)
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 1000.0,
            },
            PacingCheckpoint {
                distance_km: 1.0,
                elevation_m: 0.0,
            }, // steep downhill
            PacingCheckpoint {
                distance_km: 2.0,
                elevation_m: 0.0,
            }, // flat
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 3);
        assert!((fractions[0] - 0.0).abs() < 1e-10);
        assert!((fractions[2] - 1.0).abs() < 1e-10);

        // Steep downhill (cost 0.5*1km=0.5) should get less time than flat (cost 1.0*1km=1.0)
        let downhill_fraction = fractions[1]; // fraction after downhill segment
        let flat_fraction = fractions[2] - fractions[1]; // fraction of flat segment
        assert!(
            downhill_fraction < flat_fraction,
            "Steep downhill ({}) should get less time than flat ({})",
            downhill_fraction,
            flat_fraction
        );
        // Downhill cost = 0.5, flat cost = 1.0, total = 1.5
        // Expected fractions: 0.0, 0.5/1.5 = 0.333, 1.0
        assert!(
            (fractions[1] - 1.0 / 3.0).abs() < 1e-10,
            "Expected 1/3 for steep downhill, got {}",
            fractions[1]
        );
    }

    #[test]
    fn test_elevation_fractions_steep_uphill() {
        // Steep uphill: 500m gain over 1km -> gradient = 0.5
        // cost_factor = 1.0 + 12.0 * 0.5 = 7.0
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 0.0,
            },
            PacingCheckpoint {
                distance_km: 1.0,
                elevation_m: 500.0,
            }, // steep uphill
            PacingCheckpoint {
                distance_km: 2.0,
                elevation_m: 500.0,
            }, // flat
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 3);

        // Uphill cost = 7.0*1km=7.0, flat cost = 1.0*1km=1.0, total = 8.0
        // Expected: 0.0, 7/8=0.875, 1.0
        assert!(
            (fractions[1] - 7.0 / 8.0).abs() < 1e-10,
            "Expected 7/8 for steep uphill, got {}",
            fractions[1]
        );
    }

    #[test]
    fn test_elevation_fractions_negative_distance_delta() {
        // Non-monotonic distances (should handle gracefully with zero cost)
        let checkpoints = vec![
            PacingCheckpoint {
                distance_km: 0.0,
                elevation_m: 100.0,
            },
            PacingCheckpoint {
                distance_km: 50.0,
                elevation_m: 200.0,
            },
            PacingCheckpoint {
                distance_km: 30.0,
                elevation_m: 150.0,
            }, // backwards!
            PacingCheckpoint {
                distance_km: 90.0,
                elevation_m: 100.0,
            },
        ];
        let fractions = calculate_pass_time_fractions(&checkpoints);
        assert_eq!(fractions.len(), 4);
        assert!((fractions[0] - 0.0).abs() < 1e-10);
        assert!((fractions[3] - 1.0).abs() < 1e-10);
        // Negative-distance segment gets zero cost, so fractions[1] == fractions[2]
        assert!(
            (fractions[1] - fractions[2]).abs() < 1e-10,
            "Negative-distance segment should have zero cost"
        );
    }
}
