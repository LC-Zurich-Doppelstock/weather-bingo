//! Forecast resolution service.
//!
//! Implements the 4-step forecast resolution logic from specs.md Section 4.2,
//! including calculated fields (feels-like, precipitation type).

use chrono::{DateTime, Duration, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

use crate::db::models::{Checkpoint, Forecast};
use crate::db::queries::{self, InsertForecastParams};
use crate::errors::AppError;
use crate::services::yr::{YrClient, YrFetchResult};

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

/// Resolve the forecast for a checkpoint, implementing the 4-step logic.
///
/// 1. Calculate pass-through time
/// 2. Check DB for latest forecast
/// 3. If stale or missing, fetch from yr.no
/// 4. Return result with stale indicator
pub async fn resolve_forecast(
    pool: &PgPool,
    yr_client: &YrClient,
    checkpoint: &Checkpoint,
    forecast_time: DateTime<Utc>,
    staleness_secs: u64,
) -> Result<(Forecast, bool), AppError> {
    // Step 2: Check DB for cached forecast
    let cached = queries::get_latest_forecast(pool, checkpoint.id, forecast_time).await?;

    if let Some(ref forecast) = cached {
        let age = Utc::now() - forecast.fetched_at;
        if age.num_seconds() < staleness_secs as i64 {
            // Fresh enough, return cached
            return Ok((forecast.clone(), false));
        }
    }

    // Step 3: Fetch from yr.no
    let lat = checkpoint.latitude.to_f64().unwrap_or(0.0);
    let lon = checkpoint.longitude.to_f64().unwrap_or(0.0);
    let alt = checkpoint.elevation_m.to_f64().unwrap_or(0.0);

    match yr_client
        .fetch_forecast(lat, lon, alt, forecast_time, None)
        .await
    {
        Ok(YrFetchResult::NewData(parsed)) => {
            let temp_c = parsed.temperature_c.to_f64().unwrap_or(0.0);
            let wind_ms = parsed.wind_speed_ms.to_f64().unwrap_or(0.0);
            let precip_mm = parsed.precipitation_mm.to_f64().unwrap_or(0.0);

            let feels_like = calculate_feels_like(temp_c, wind_ms);
            let precip_type = infer_precipitation_type(&parsed.symbol_code, temp_c, precip_mm);

            let feels_like_dec =
                Decimal::from_str(&format!("{:.1}", feels_like)).unwrap_or_default();

            let forecast = queries::insert_forecast(
                pool,
                InsertForecastParams {
                    checkpoint_id: checkpoint.id,
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
                    raw_response: parsed.raw_response,
                },
            )
            .await?;

            Ok((forecast, false))
        }
        Ok(YrFetchResult::NotModified) => {
            // Data hasn't changed, return cached if available
            if let Some(forecast) = cached {
                Ok((forecast, false))
            } else {
                Err(AppError::ExternalServiceError(
                    "yr.no returned 304 but no cached data exists".to_string(),
                ))
            }
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

/// Resolve a checkpoint by ID from the database.
pub async fn get_checkpoint(pool: &PgPool, checkpoint_id: Uuid) -> Result<Checkpoint, AppError> {
    // Query the checkpoint directly
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
        // 13.12 + 0.6215*(-4) - 11.37*(11.52^0.16) + 0.3965*(-4)*(11.52^0.16)
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
        // Unknown symbol, cold temp -> snow
        assert_eq!(infer_precipitation_type("cloudy", -3.0, 1.0), "snow");
    }

    #[test]
    fn test_precip_type_fallback_warm() {
        // Unknown symbol, warm temp -> rain
        assert_eq!(infer_precipitation_type("cloudy", 5.0, 1.0), "rain");
    }

    #[test]
    fn test_precip_type_fallback_borderline() {
        // Unknown symbol, 0-2°C -> sleet
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
