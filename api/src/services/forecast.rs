//! Forecast resolution service.
//!
//! Extracts only the forecast entry closest to the requested pass-through time
//! from yr.no's timeseries response, and stores it in the forecasts table.
//!
//! Performance-optimised: uses yr_responses cache (keyed by location) with
//! yr.no's Expires header for freshness, If-Modified-Since for conditional
//! requests. Only the relevant forecast(s) are extracted and stored per request.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
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

/// Location key for grouping checkpoints by yr.no coordinate grid.
/// yr.no rounds to 4 decimal places, so we use the Decimal values from the DB.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct LocationKey {
    latitude: Decimal,
    longitude: Decimal,
    elevation_m: Decimal,
}

/// Ensure we have a valid (non-expired) yr.no timeseries for a given location,
/// and extract + store only the forecast entries for the requested times.
///
/// `targets` is a list of `(checkpoint_id, forecast_time)` pairs — one forecast
/// entry will be extracted from the yr.no response for each pair (snapped to the
/// nearest yr.no time slot).
///
/// Returns `Ok(true)` if fresh data was fetched and stored, `Ok(false)` if
/// the cache was still valid (no new data needed).
async fn ensure_yr_timeseries(
    pool: &PgPool,
    yr_client: &YrClient,
    lat_dec: Decimal,
    lon_dec: Decimal,
    ele_dec: Decimal,
    targets: &[(Uuid, DateTime<Utc>)],
) -> Result<bool, AppError> {
    // 1. Check for a non-expired cached response
    if queries::get_yr_cached_response(pool, lat_dec, lon_dec, ele_dec)
        .await?
        .is_some()
    {
        return Ok(false);
    }

    // 2. Cache miss or expired — try conditional request with If-Modified-Since
    let existing = queries::get_yr_cached_response_any(pool, lat_dec, lon_dec, ele_dec).await?;
    let if_modified_since = existing.as_ref().and_then(|c| c.last_modified.as_deref());

    let lat = lat_dec.to_f64().unwrap_or(0.0);
    let lon = lon_dec.to_f64().unwrap_or(0.0);
    let alt = ele_dec.to_f64().unwrap_or(0.0);

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
                lat_dec,
                lon_dec,
                ele_dec,
                Utc::now(),
                expires_at,
                last_modified.as_deref(),
                &raw_json,
            )
            .await?;

            // Extract only the forecast entries for the requested times
            let forecast_times: Vec<DateTime<Utc>> =
                targets.iter().map(|(_, ft)| *ft).collect();
            let parsed = extract_forecasts_at_times(&raw_json, &forecast_times)?;
            let now = Utc::now();

            for (i, (cp_id, target_time)) in targets.iter().enumerate() {
                if let Some(ref forecast) = parsed[i] {
                    let params = build_single_insert_params(*cp_id, forecast, now);
                    queries::insert_forecast(pool, params).await?;
                    tracing::debug!(
                        "Inserted forecast for checkpoint {} at {}",
                        cp_id,
                        forecast.forecast_time,
                    );
                } else {
                    tracing::debug!(
                        "No in-range yr.no data for checkpoint {} at {} — skipping insert",
                        cp_id,
                        target_time,
                    );
                }
            }

            Ok(true)
        }
        YrTimeseriesResult::NotModified => {
            // yr.no says data unchanged — bump the expires_at on the existing cache
            if let Some(cached) = existing {
                let new_expires = Utc::now() + Duration::hours(1);
                queries::upsert_yr_cached_response(
                    pool,
                    lat_dec,
                    lon_dec,
                    ele_dec,
                    cached.fetched_at,
                    new_expires,
                    cached.last_modified.as_deref(),
                    &cached.raw_response,
                )
                .await?;
                Ok(false)
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

/// Resolve the forecast for a single checkpoint.
///
/// Ensures the yr.no timeseries is fresh (extracts and inserts only the
/// targeted forecast entry if new data), then queries the DB for the nearest
/// stored forecast to the requested time.
///
/// Returns `(Some(forecast), is_stale)` when a forecast is available, or
/// `(None, false)` when yr.no doesn't cover the requested time (e.g. race
/// date is beyond the ~10-day forecast horizon).
pub async fn resolve_forecast(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoint: &Checkpoint,
    forecast_time: DateTime<Utc>,
    staleness_secs: u64,
) -> Result<(Option<Forecast>, bool), AppError> {
    // Step 1: Check DB for cached forecast while yr cache is valid
    let cached = queries::get_latest_forecast(pool, checkpoint.id, forecast_time).await?;

    if let Some(ref forecast) = cached {
        let yr_valid = queries::is_yr_cache_valid(
            pool,
            checkpoint.latitude,
            checkpoint.longitude,
            checkpoint.elevation_m,
        )
        .await?;

        if yr_valid {
            return Ok((Some(forecast.clone()), false));
        }

        // Fallback: staleness check
        let age = Utc::now() - forecast.fetched_at;
        if age.num_seconds() < staleness_secs as i64 {
            return Ok((Some(forecast.clone()), false));
        }
    }

    // Step 2: Ensure yr.no timeseries is fresh + insert targeted forecast
    match ensure_yr_timeseries(
        pool,
        yr_client,
        checkpoint.latitude,
        checkpoint.longitude,
        checkpoint.elevation_m,
        &[(checkpoint.id, forecast_time)],
    )
    .await
    {
        Ok(_) => {
            // Query DB for the nearest stored forecast.
            // If Nothing was inserted (out-of-range), this returns None — that's OK.
            let forecast = queries::get_latest_forecast(pool, checkpoint.id, forecast_time).await?;
            Ok((forecast, false))
        }
        Err(e) => {
            // yr.no failed — return stale cache if available
            if let Some(forecast) = cached {
                tracing::warn!("yr.no unavailable, returning stale data: {}", e);
                Ok((Some(forecast), true))
            } else {
                Err(AppError::ExternalServiceError(format!(
                    "yr.no unavailable and no cached data: {}",
                    e
                )))
            }
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

/// Resolve forecasts for multiple checkpoints in a race, efficiently.
///
/// Targeted extraction: only the forecast entry closest to each checkpoint's
/// pacing-derived pass-through time is extracted from yr.no and stored.
///   1. One query to fetch the latest forecast for ALL checkpoints (nearest to pacing time)
///   2. One query to check yr.no cache validity for ALL locations
///   3. Only for stale/missing: parallel yr.no fetches + targeted extraction + insert
///   4. Re-query DB for any checkpoints that needed fresh data
///
/// Warm-cache happy path: **2 DB queries** total (regardless of checkpoint count).
pub async fn resolve_race_forecasts(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoints: &[CheckpointWithTime],
    staleness_secs: u64,
) -> Result<Vec<ResolvedForecast>, AppError> {
    let n = checkpoints.len();

    // ── Step 1: Batch-fetch latest forecasts for all checkpoints (1 query) ──
    let pairs: Vec<(Uuid, DateTime<Utc>)> = checkpoints
        .iter()
        .map(|cpwt| (cpwt.checkpoint.id, cpwt.forecast_time))
        .collect();

    let cached_forecasts = queries::get_latest_forecasts_batch(pool, &pairs).await?;

    // ── Step 2: Batch-check yr.no cache validity for all locations (1 query) ──
    let locations: Vec<(Decimal, Decimal, Decimal)> = checkpoints
        .iter()
        .map(|cpwt| {
            (
                cpwt.checkpoint.latitude,
                cpwt.checkpoint.longitude,
                cpwt.checkpoint.elevation_m,
            )
        })
        .collect();

    let unique_locations: Vec<(Decimal, Decimal, Decimal)> = {
        let mut seen = std::collections::HashSet::new();
        locations
            .iter()
            .filter(|loc| seen.insert(**loc))
            .copied()
            .collect()
    };

    let valid_locations = queries::get_valid_yr_cache_locations(pool, &unique_locations).await?;
    let valid_set: std::collections::HashSet<(Decimal, Decimal, Decimal)> =
        valid_locations.into_iter().collect();

    // ── Step 3: Classify each checkpoint ──
    let mut results: Vec<Option<ResolvedForecast>> = vec![None; n];
    let mut need_yr: Vec<usize> = Vec::new();

    for i in 0..n {
        if let Some(ref forecast) = cached_forecasts[i] {
            let loc = &locations[i];

            if valid_set.contains(loc) {
                results[i] = Some(ResolvedForecast {
                    forecast: Some(forecast.clone()),
                    is_stale: false,
                });
                continue;
            }

            // Fallback: staleness check
            let age = Utc::now() - forecast.fetched_at;
            if age.num_seconds() < staleness_secs as i64 {
                results[i] = Some(ResolvedForecast {
                    forecast: Some(forecast.clone()),
                    is_stale: false,
                });
                continue;
            }
        }

        need_yr.push(i);
    }

    if need_yr.is_empty() {
        return Ok(results.into_iter().map(|r| r.unwrap()).collect());
    }

    // ── Step 4: Group stale checkpoints by location, fetch + insert targeted forecasts ──
    let mut location_groups: HashMap<LocationKey, Vec<usize>> = HashMap::new();
    for &idx in &need_yr {
        let cp = &checkpoints[idx].checkpoint;
        let key = LocationKey {
            latitude: cp.latitude,
            longitude: cp.longitude,
            elevation_m: cp.elevation_m,
        };
        location_groups.entry(key).or_default().push(idx);
    }

    let mut fetch_futures = Vec::new();
    let location_keys: Vec<LocationKey> = location_groups.keys().cloned().collect();

    for key in &location_keys {
        let pool = pool.clone();
        let yr_client = yr_client.clone();
        let lat = key.latitude;
        let lon = key.longitude;
        let ele = key.elevation_m;

        // Collect (checkpoint_id, forecast_time) pairs at this location
        let targets: Vec<(Uuid, DateTime<Utc>)> = location_groups[key]
            .iter()
            .map(|&idx| (checkpoints[idx].checkpoint.id, checkpoints[idx].forecast_time))
            .collect();

        fetch_futures.push(async move {
            ensure_yr_timeseries(&pool, &yr_client, lat, lon, ele, &targets).await
        });
    }

    let fetch_results = futures::future::join_all(fetch_futures).await;

    // ── Step 5: Re-query DB for checkpoints that needed fresh data ──
    for (loc_idx, fetch_result) in fetch_results.into_iter().enumerate() {
        let key = &location_keys[loc_idx];
        let cp_indices = &location_groups[key];

        match fetch_result {
            Ok(_) => {
                // Data was fetched and inserted (or skipped if out-of-range).
                // Query DB for each checkpoint's nearest forecast to its pacing time.
                for &cp_idx in cp_indices {
                    let cpwt = &checkpoints[cp_idx];
                    let forecast =
                        queries::get_latest_forecast(pool, cpwt.checkpoint.id, cpwt.forecast_time)
                            .await?;

                    // forecast is None when yr.no doesn't cover the requested time
                    // (e.g. race date is beyond the ~10-day forecast horizon).
                    // This is not an error — we return it as "forecast not available".
                    results[cp_idx] = Some(ResolvedForecast {
                        forecast,
                        is_stale: false,
                    });
                }
            }
            Err(e) => {
                // yr.no failed — return stale data from the batch we already fetched
                for &cp_idx in cp_indices {
                    let cpwt = &checkpoints[cp_idx];

                    if let Some(ref forecast) = cached_forecasts[cp_idx] {
                        tracing::warn!(
                            "yr.no unavailable for location ({}, {}), returning stale data: {}",
                            key.latitude,
                            key.longitude,
                            e
                        );
                        results[cp_idx] = Some(ResolvedForecast {
                            forecast: Some(forecast.clone()),
                            is_stale: true,
                        });
                    } else {
                        return Err(AppError::ExternalServiceError(format!(
                            "yr.no unavailable for checkpoint {} and no cached data: {}",
                            cpwt.checkpoint.name, e
                        )));
                    }
                }
            }
        }
    }

    Ok(results.into_iter().map(|r| r.unwrap()).collect())
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
}
