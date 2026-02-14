//! Forecast resolution service.
//!
//! Implements the 4-step forecast resolution logic from specs.md Section 4.2,
//! including calculated fields (feels-like, precipitation type).
//!
//! Performance-optimised: uses yr_responses cache (keyed by location) with
//! yr.no's Expires header for freshness, If-Modified-Since for conditional
//! requests, and batch extraction of multiple forecasts from one timeseries.

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
    extract_forecast_at_time, parse_expires_header, YrClient, YrTimeseriesResult,
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
pub fn calculate_pass_time(
    start_time: DateTime<Utc>,
    checkpoint_distance_km: f64,
    race_distance_km: f64,
    target_duration_hours: f64,
) -> DateTime<Utc> {
    let fraction = checkpoint_distance_km / race_distance_km;
    let duration_secs = (target_duration_hours * 3600.0 * fraction) as i64;
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

/// Ensure we have a valid (non-expired) yr.no timeseries for a given location.
/// Returns the raw JSON from the cache or from a fresh fetch.
async fn ensure_yr_timeseries(
    pool: &PgPool,
    yr_client: &YrClient,
    lat_dec: Decimal,
    lon_dec: Decimal,
    ele_dec: Decimal,
) -> Result<serde_json::Value, AppError> {
    // 1. Check for a non-expired cached response
    if let Some(cached) = queries::get_yr_cached_response(pool, lat_dec, lon_dec, ele_dec).await? {
        return Ok(cached.raw_response);
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

            Ok(raw_json)
        }
        YrTimeseriesResult::NotModified => {
            // yr.no says data unchanged — bump the expires_at on the existing cache
            if let Some(cached) = existing {
                // Bump expiry by 1 hour since yr.no confirmed no change
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
                Ok(cached.raw_response)
            } else {
                Err(AppError::ExternalServiceError(
                    "yr.no returned 304 but no cached data exists".to_string(),
                ))
            }
        }
    }
}

/// Resolve the forecast for a single checkpoint.
///
/// Uses the yr_responses cache with Expires-based freshness.
/// Falls back to `forecast_staleness_secs` if yr_responses cache is unavailable.
pub async fn resolve_forecast(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoint: &Checkpoint,
    forecast_time: DateTime<Utc>,
    staleness_secs: u64,
) -> Result<(Forecast, bool), AppError> {
    // Step 1: Check DB for cached forecast
    let cached = queries::get_latest_forecast(pool, checkpoint.id, forecast_time).await?;

    // If we have a fresh-enough DB forecast, return it without hitting yr.no at all.
    // "Fresh enough" now means: the yr_responses cache for this location hasn't expired
    // (checked inside ensure_yr_timeseries), OR as a fallback, the forecast is younger
    // than staleness_secs.
    if let Some(ref forecast) = cached {
        // Quick check: is the yr_responses cache still valid for this location?
        let yr_cache = queries::get_yr_cached_response(
            pool,
            checkpoint.latitude,
            checkpoint.longitude,
            checkpoint.elevation_m,
        )
        .await?;

        if yr_cache.is_some() {
            // yr.no data hasn't expired yet, so our DB forecast is current
            return Ok((forecast.clone(), false));
        }

        // Fallback: use staleness_secs
        let age = Utc::now() - forecast.fetched_at;
        if age.num_seconds() < staleness_secs as i64 {
            return Ok((forecast.clone(), false));
        }
    }

    // Step 2: Get (possibly fresh) yr.no timeseries
    match ensure_yr_timeseries(
        pool,
        yr_client,
        checkpoint.latitude,
        checkpoint.longitude,
        checkpoint.elevation_m,
    )
    .await
    {
        Ok(raw_json) => {
            let parsed = extract_forecast_at_time(&raw_json, forecast_time)?;
            let forecast =
                insert_parsed_forecast(pool, checkpoint.id, forecast_time, &parsed).await?;
            Ok((forecast, false))
        }
        Err(e) => {
            // yr.no failed — return stale cache if available
            if let Some(forecast) = cached {
                tracing::warn!("yr.no unavailable, returning stale data: {}", e);
                Ok((forecast, true))
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
#[allow(dead_code)]
pub struct ResolvedForecast {
    pub checkpoint_id: Uuid,
    pub forecast: Forecast,
    pub is_stale: bool,
    pub forecast_time: DateTime<Utc>,
}

/// Resolve forecasts for multiple checkpoints in a race, efficiently.
///
/// Groups checkpoints by location (lat/lon/elevation), makes one yr.no call
/// per unique location (in parallel), then extracts individual forecasts from
/// each cached timeseries.
pub async fn resolve_race_forecasts(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoints: &[CheckpointWithTime],
    staleness_secs: u64,
) -> Result<Vec<ResolvedForecast>, AppError> {
    // 1. Check which checkpoints already have fresh DB forecasts
    let mut results: Vec<Option<ResolvedForecast>> = (0..checkpoints.len()).map(|_| None).collect();
    let mut need_yr: Vec<usize> = Vec::new(); // indices into `checkpoints` that need yr.no data

    for (i, cpwt) in checkpoints.iter().enumerate() {
        let cached =
            queries::get_latest_forecast(pool, cpwt.checkpoint.id, cpwt.forecast_time).await?;

        if let Some(ref forecast) = cached {
            // Check if yr_responses cache is still valid
            let yr_cache = queries::get_yr_cached_response(
                pool,
                cpwt.checkpoint.latitude,
                cpwt.checkpoint.longitude,
                cpwt.checkpoint.elevation_m,
            )
            .await?;

            if yr_cache.is_some() {
                results[i] = Some(ResolvedForecast {
                    checkpoint_id: cpwt.checkpoint.id,
                    forecast: forecast.clone(),
                    is_stale: false,
                    forecast_time: cpwt.forecast_time,
                });
                continue;
            }

            // Fallback: staleness check
            let age = Utc::now() - forecast.fetched_at;
            if age.num_seconds() < staleness_secs as i64 {
                results[i] = Some(ResolvedForecast {
                    checkpoint_id: cpwt.checkpoint.id,
                    forecast: forecast.clone(),
                    is_stale: false,
                    forecast_time: cpwt.forecast_time,
                });
                continue;
            }
        }

        need_yr.push(i);
    }

    if need_yr.is_empty() {
        // All from cache
        return Ok(results.into_iter().map(|r| r.unwrap()).collect());
    }

    // 2. Group checkpoints needing yr.no data by location
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

    // 3. Fetch yr.no timeseries in parallel for each unique location
    let mut fetch_futures = Vec::new();
    let location_keys: Vec<LocationKey> = location_groups.keys().cloned().collect();

    for key in &location_keys {
        let pool = pool.clone();
        let yr_client = yr_client.clone();
        let lat = key.latitude;
        let lon = key.longitude;
        let ele = key.elevation_m;

        fetch_futures
            .push(async move { ensure_yr_timeseries(&pool, &yr_client, lat, lon, ele).await });
    }

    let fetch_results = futures::future::join_all(fetch_futures).await;

    // 4. For each location, extract forecasts for all checkpoints at that location
    for (loc_idx, fetch_result) in fetch_results.into_iter().enumerate() {
        let key = &location_keys[loc_idx];
        let cp_indices = &location_groups[key];

        match fetch_result {
            Ok(raw_json) => {
                // Collect forecast times for batch extraction
                let forecast_times: Vec<DateTime<Utc>> = cp_indices
                    .iter()
                    .map(|&idx| checkpoints[idx].forecast_time)
                    .collect();

                let parsed_forecasts =
                    crate::services::yr::extract_forecasts_at_times(&raw_json, &forecast_times)?;

                for (j, &cp_idx) in cp_indices.iter().enumerate() {
                    let forecast = insert_parsed_forecast(
                        pool,
                        checkpoints[cp_idx].checkpoint.id,
                        checkpoints[cp_idx].forecast_time,
                        &parsed_forecasts[j],
                    )
                    .await?;

                    results[cp_idx] = Some(ResolvedForecast {
                        checkpoint_id: checkpoints[cp_idx].checkpoint.id,
                        forecast,
                        is_stale: false,
                        forecast_time: checkpoints[cp_idx].forecast_time,
                    });
                }
            }
            Err(e) => {
                // yr.no failed for this location — try stale cache for each checkpoint
                for &cp_idx in cp_indices {
                    let cpwt = &checkpoints[cp_idx];
                    let cached =
                        queries::get_latest_forecast(pool, cpwt.checkpoint.id, cpwt.forecast_time)
                            .await?;

                    if let Some(forecast) = cached {
                        tracing::warn!(
                            "yr.no unavailable for location ({}, {}), returning stale data: {}",
                            key.latitude,
                            key.longitude,
                            e
                        );
                        results[cp_idx] = Some(ResolvedForecast {
                            checkpoint_id: cpwt.checkpoint.id,
                            forecast,
                            is_stale: true,
                            forecast_time: cpwt.forecast_time,
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

/// Compute derived fields and insert a parsed yr.no forecast into the DB.
async fn insert_parsed_forecast(
    pool: &PgPool,
    checkpoint_id: Uuid,
    forecast_time: DateTime<Utc>,
    parsed: &crate::services::yr::YrParsedForecast,
) -> Result<Forecast, AppError> {
    let temp_c = parsed.temperature_c.to_f64().unwrap_or(0.0);
    let wind_ms = parsed.wind_speed_ms.to_f64().unwrap_or(0.0);
    let precip_mm = parsed.precipitation_mm.to_f64().unwrap_or(0.0);

    let feels_like = calculate_feels_like(temp_c, wind_ms);
    let precip_type = infer_precipitation_type(&parsed.symbol_code, temp_c, precip_mm);

    let feels_like_dec = Decimal::from_str(&format!("{:.1}", feels_like)).unwrap_or_default();

    let forecast = queries::insert_forecast(
        pool,
        InsertForecastParams {
            checkpoint_id,
            forecast_time,
            fetched_at: Utc::now(),
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
        },
    )
    .await?;

    Ok(forecast)
}

/// Resolve a checkpoint by ID from the database.
pub async fn get_checkpoint(pool: &PgPool, checkpoint_id: Uuid) -> Result<Checkpoint, AppError> {
    let checkpoint = sqlx::query_as::<_, Checkpoint>(
        "SELECT id, race_id, name, distance_km, latitude, longitude, elevation_m, sort_order
         FROM checkpoints WHERE id = $1",
    )
    .bind(checkpoint_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Checkpoint {} not found", checkpoint_id)))?;

    Ok(checkpoint)
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
}
