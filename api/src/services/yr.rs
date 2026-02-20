//! yr.no Locationforecast 2.0 client.
//!
//! Fetches weather forecasts from the MET Norway API.
//! See: https://api.met.no/weatherapi/locationforecast/2.0/documentation

use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, IF_MODIFIED_SINCE, USER_AGENT};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::errors::AppError;
use crate::helpers::{f64_to_decimal_1dp, opt_f64_to_decimal_1dp};

const YR_API_URL: &str = "https://api.met.no/weatherapi/locationforecast/2.0/complete";
/// HTTP request timeout for yr.no API calls (seconds).
const YR_HTTP_TIMEOUT_SECS: u64 = 30;

/// Temporal resolution of a yr.no timeseries entry, determined by which
/// period blocks (`next_1_hours` / `next_6_hours`) are present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastResolution {
    /// Short-range: `next_1_hours` present, data at 1-hour intervals.
    Hourly,
    /// Medium-range: only `next_6_hours` present, data at 6-hour intervals.
    SixHourly,
}

impl ForecastResolution {
    /// Maximum acceptable time difference (in seconds) between a requested time
    /// and the closest yr.no entry for this resolution tier.
    ///
    /// - Hourly:    1 hour  (3 600 s)
    /// - SixHourly: 3 hours (10 800 s)
    pub fn max_tolerance_secs(self) -> i64 {
        match self {
            ForecastResolution::Hourly => 3_600,
            ForecastResolution::SixHourly => 10_800,
        }
    }
}

/// Client for the yr.no Locationforecast API.
#[derive(Debug, Clone)]
pub struct YrClient {
    client: reqwest::Client,
    user_agent: String,
}

/// The result of a yr.no timeseries fetch.
pub enum YrTimeseriesResult {
    /// New timeseries data received (HTTP 200).
    NewData {
        /// Full raw JSON response (stored in yr_responses table).
        raw_json: serde_json::Value,
        /// yr.no `Expires` header — when this data becomes stale.
        expires: Option<String>,
        /// yr.no `Last-Modified` header — for conditional requests.
        last_modified: Option<String>,
    },
    /// Data not modified since last fetch (HTTP 304).
    /// Carries any Expires/Last-Modified headers from the 304 response.
    NotModified {
        /// yr.no `Expires` header from the 304 response.
        expires: Option<String>,
        /// yr.no `Last-Modified` header from the 304 response.
        last_modified: Option<String>,
    },
}

/// Parsed forecast data from yr.no for a specific time.
/// Extracted from a cached timeseries, not fetched directly.
#[derive(Debug, Clone)]
pub struct YrParsedForecast {
    /// The yr.no native timeseries timestamp for this entry.
    /// This is the time slot from yr.no (e.g. "2026-03-01T07:00:00Z"),
    /// NOT the pacing-derived pass-through time.
    pub forecast_time: DateTime<Utc>,
    pub temperature_c: Decimal,
    pub temperature_percentile_10_c: Option<Decimal>,
    pub temperature_percentile_90_c: Option<Decimal>,
    pub wind_speed_ms: Decimal,
    pub wind_speed_percentile_10_ms: Option<Decimal>,
    pub wind_speed_percentile_90_ms: Option<Decimal>,
    pub wind_direction_deg: Decimal,
    pub wind_gust_ms: Option<Decimal>,
    pub precipitation_mm: Decimal,
    pub precipitation_min_mm: Option<Decimal>,
    pub precipitation_max_mm: Option<Decimal>,
    pub humidity_pct: Decimal,
    pub dew_point_c: Decimal,
    pub cloud_cover_pct: Decimal,
    pub uv_index: Option<Decimal>,
    pub symbol_code: String,
    /// When yr.no's weather model generated this forecast (`properties.meta.updated_at`).
    /// `None` if the meta block is missing or unparseable.
    pub yr_model_run_at: Option<DateTime<Utc>>,
    /// Temporal resolution of this timeseries entry.
    pub resolution: ForecastResolution,
}

/// Result of extracting forecasts from a yr.no cached response.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// One `Option<YrParsedForecast>` per requested time.
    pub forecasts: Vec<Option<YrParsedForecast>>,
    /// The furthest timestamp in the yr.no timeseries — the actual forecast horizon.
    pub forecast_horizon: DateTime<Utc>,
}

// --- yr.no JSON response types ---

#[derive(Debug, Deserialize)]
struct YrResponse {
    properties: YrProperties,
}

#[derive(Debug, Deserialize)]
struct YrMeta {
    /// When the yr.no weather model generated this forecast.
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YrProperties {
    meta: Option<YrMeta>,
    timeseries: Vec<YrTimeseries>,
}

#[derive(Debug, Deserialize)]
struct YrTimeseries {
    time: String,
    data: YrData,
}

#[derive(Debug, Deserialize)]
struct YrData {
    instant: YrInstant,
    next_1_hours: Option<YrPeriod>,
    next_6_hours: Option<YrPeriod>,
}

#[derive(Debug, Deserialize)]
struct YrInstant {
    details: YrInstantDetails,
}

#[derive(Debug, Deserialize)]
struct YrInstantDetails {
    air_temperature: Option<f64>,
    air_temperature_percentile_10: Option<f64>,
    air_temperature_percentile_90: Option<f64>,
    wind_speed: Option<f64>,
    wind_speed_percentile_10: Option<f64>,
    wind_speed_percentile_90: Option<f64>,
    wind_from_direction: Option<f64>,
    wind_speed_of_gust: Option<f64>,
    relative_humidity: Option<f64>,
    dew_point_temperature: Option<f64>,
    cloud_area_fraction: Option<f64>,
    ultraviolet_index_clear_sky: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct YrPeriod {
    summary: Option<YrSummary>,
    details: Option<YrPeriodDetails>,
}

#[derive(Debug, Deserialize)]
struct YrSummary {
    symbol_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YrPeriodDetails {
    precipitation_amount: Option<f64>,
    precipitation_amount_min: Option<f64>,
    precipitation_amount_max: Option<f64>,
}

fn f64_to_decimal(v: f64) -> Decimal {
    f64_to_decimal_1dp(v)
}

fn opt_f64_to_decimal(v: Option<f64>) -> Option<Decimal> {
    opt_f64_to_decimal_1dp(v)
}

impl YrClient {
    pub fn new(user_agent: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(YR_HTTP_TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            user_agent: user_agent.to_string(),
        }
    }

    /// Fetch the full timeseries from yr.no for a given location.
    ///
    /// Returns the raw JSON and caching headers. The caller is responsible
    /// for storing this in `yr_responses` and extracting individual forecasts.
    pub async fn fetch_timeseries(
        &self,
        lat: f64,
        lon: f64,
        altitude: f64,
        if_modified_since: Option<&str>,
    ) -> Result<YrTimeseriesResult, AppError> {
        // Limit to 4 decimal places per yr.no terms of service
        let lat_str = format!("{:.4}", lat);
        let lon_str = format!("{:.4}", lon);
        let alt_str = format!("{:.0}", altitude);

        let url = format!(
            "{}?lat={}&lon={}&altitude={}",
            YR_API_URL, lat_str, lon_str, alt_str
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.user_agent)
                .map_err(|e| AppError::InternalError(format!("Invalid User-Agent: {}", e)))?,
        );

        if let Some(ims) = if_modified_since {
            if let Ok(val) = HeaderValue::from_str(ims) {
                headers.insert(IF_MODIFIED_SINCE, val);
            }
        }

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("yr.no request failed: {}", e)))?;

        // Handle 304 Not Modified — extract headers before discarding the response
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            let expires = response
                .headers()
                .get("expires")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let last_modified = response
                .headers()
                .get("last-modified")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            return Ok(YrTimeseriesResult::NotModified {
                expires,
                last_modified,
            });
        }

        if !response.status().is_success() {
            return Err(AppError::ExternalServiceError(format!(
                "yr.no returned HTTP {}",
                response.status()
            )));
        }

        // Extract caching headers before consuming the body
        let expires = response
            .headers()
            .get("expires")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let last_modified = response
            .headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Parse JSON once directly into serde_json::Value (stored in DB).
        // We deserialize into typed structs only when extracting forecasts.
        let raw_json: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError(format!("yr.no JSON parse error: {}", e))
        })?;

        Ok(YrTimeseriesResult::NewData {
            raw_json,
            expires,
            last_modified,
        })
    }
}

/// Extract forecasts for multiple times from a single cached yr.no timeseries.
///
/// Returns an `ExtractionResult` containing:
/// - One `Option<YrParsedForecast>` per requested time:
///   - `Some(forecast)` if yr.no has a timeseries entry within the resolution-appropriate
///     tolerance (1h for hourly data, 3h for 6-hourly data).
///   - `None` if the closest entry is too far away (e.g. the requested time is beyond
///     yr.no's forecast horizon).
/// - The `forecast_horizon`: the last (furthest future) timestamp in the yr.no timeseries.
///
/// Much more efficient than calling `extract_forecast_at_time` N times because
/// we deserialize the JSON only once.
pub fn extract_forecasts_at_times(
    raw_json: serde_json::Value,
    forecast_times: &[DateTime<Utc>],
) -> Result<ExtractionResult, AppError> {
    let yr_response: YrResponse = serde_json::from_value(raw_json).map_err(|e| {
        AppError::ExternalServiceError(format!("yr.no response structure error: {}", e))
    })?;

    // Parse model run timestamp from meta.updated_at
    let yr_model_run_at = yr_response
        .properties
        .meta
        .as_ref()
        .and_then(|m| m.updated_at.as_deref())
        .and_then(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });

    let timeseries = &yr_response.properties.timeseries;
    if timeseries.is_empty() {
        return Err(AppError::ExternalServiceError(
            "yr.no returned empty timeseries".to_string(),
        ));
    }

    // Pre-parse all timeseries timestamps once (avoiding redundant RFC3339 parsing per query).
    // Entries with unparseable timestamps are skipped (logged) rather than defaulting to epoch 0.
    let parsed_entries: Vec<(i64, &YrTimeseries)> = timeseries
        .iter()
        .filter_map(|ts| match chrono::DateTime::parse_from_rfc3339(&ts.time) {
            Ok(dt) => Some((dt.timestamp(), ts)),
            Err(e) => {
                tracing::warn!(
                    "Skipping yr.no timeseries entry with unparseable time '{}': {}",
                    ts.time,
                    e,
                );
                None
            }
        })
        .collect();

    if parsed_entries.is_empty() {
        return Err(AppError::ExternalServiceError(
            "yr.no timeseries has no entries with valid timestamps".to_string(),
        ));
    }

    // The last entry's timestamp is the actual forecast horizon
    let forecast_horizon = {
        let last_ts = parsed_entries.last().unwrap().0;
        DateTime::<Utc>::from_timestamp(last_ts, 0).ok_or_else(|| {
            AppError::ExternalServiceError(
                "yr.no last timeseries timestamp out of range".to_string(),
            )
        })?
    };

    let mut results = Vec::with_capacity(forecast_times.len());

    for &ft in forecast_times {
        let target_ts = ft.timestamp();
        let closest = parsed_entries
            .iter()
            .min_by_key(|(ts_time, _)| (*ts_time - target_ts).unsigned_abs())
            .map(|(_, entry)| *entry)
            .ok_or_else(|| {
                AppError::ExternalServiceError("yr.no returned empty timeseries".to_string())
            })?;

        let mut parsed = parse_timeseries_entry(closest)?;
        parsed.yr_model_run_at = yr_model_run_at;

        // Check if the closest entry is within the resolution-appropriate tolerance
        let distance_secs = (parsed.forecast_time.timestamp() - target_ts).unsigned_abs() as i64;
        let tolerance = parsed.resolution.max_tolerance_secs();

        if distance_secs > tolerance {
            tracing::debug!(
                "Closest yr.no entry to {} is {} ({} secs away, tolerance {} secs for {:?}) — skipping",
                ft,
                parsed.forecast_time,
                distance_secs,
                tolerance,
                parsed.resolution,
            );
            results.push(None);
        } else {
            results.push(Some(parsed));
        }
    }

    Ok(ExtractionResult {
        forecasts: results,
        forecast_horizon,
    })
}

/// Parse a single yr.no timeseries entry into a `YrParsedForecast`.
fn parse_timeseries_entry(entry: &YrTimeseries) -> Result<YrParsedForecast, AppError> {
    let entry_time = DateTime::parse_from_rfc3339(&entry.time)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            AppError::ExternalServiceError(format!(
                "yr.no timeseries entry has invalid time '{}': {}",
                entry.time, e
            ))
        })?;

    let instant = &entry.data.instant.details;

    // Detect temporal resolution from which period blocks are present
    let resolution = if entry.data.next_1_hours.is_some() {
        ForecastResolution::Hourly
    } else {
        // next_6_hours only, or end-of-series (both None) — treat as 6-hourly
        ForecastResolution::SixHourly
    };

    // Get period data (prefer next_1_hours, fall back to next_6_hours)
    let period = entry
        .data
        .next_1_hours
        .as_ref()
        .or(entry.data.next_6_hours.as_ref());

    let symbol_code = period
        .and_then(|p| p.summary.as_ref())
        .and_then(|s| s.symbol_code.as_ref())
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    let precip = period.and_then(|p| p.details.as_ref());

    // Helper to log warnings for mandatory fields that are unexpectedly missing.
    // yr.no should always provide these, so missing values indicate a data issue.
    let unwrap_or_warn = |field: Option<f64>, name: &str| -> f64 {
        match field {
            Some(v) => v,
            None => {
                tracing::warn!(
                    "yr.no entry at {} is missing mandatory field '{}', defaulting to 0.0",
                    entry.time,
                    name,
                );
                0.0
            }
        }
    };

    Ok(YrParsedForecast {
        forecast_time: entry_time,
        temperature_c: f64_to_decimal(unwrap_or_warn(instant.air_temperature, "air_temperature")),
        temperature_percentile_10_c: opt_f64_to_decimal(instant.air_temperature_percentile_10),
        temperature_percentile_90_c: opt_f64_to_decimal(instant.air_temperature_percentile_90),
        wind_speed_ms: f64_to_decimal(unwrap_or_warn(instant.wind_speed, "wind_speed")),
        wind_speed_percentile_10_ms: opt_f64_to_decimal(instant.wind_speed_percentile_10),
        wind_speed_percentile_90_ms: opt_f64_to_decimal(instant.wind_speed_percentile_90),
        wind_direction_deg: f64_to_decimal(unwrap_or_warn(
            instant.wind_from_direction,
            "wind_from_direction",
        )),
        wind_gust_ms: opt_f64_to_decimal(instant.wind_speed_of_gust),
        precipitation_mm: f64_to_decimal(
            precip.and_then(|p| p.precipitation_amount).unwrap_or(0.0),
        ),
        precipitation_min_mm: opt_f64_to_decimal(precip.and_then(|p| p.precipitation_amount_min)),
        precipitation_max_mm: opt_f64_to_decimal(precip.and_then(|p| p.precipitation_amount_max)),
        humidity_pct: f64_to_decimal(unwrap_or_warn(
            instant.relative_humidity,
            "relative_humidity",
        )),
        dew_point_c: f64_to_decimal(unwrap_or_warn(
            instant.dew_point_temperature,
            "dew_point_temperature",
        )),
        cloud_cover_pct: f64_to_decimal(unwrap_or_warn(
            instant.cloud_area_fraction,
            "cloud_area_fraction",
        )),
        uv_index: opt_f64_to_decimal(instant.ultraviolet_index_clear_sky),
        symbol_code,
        // Set to None here; overwritten by callers after parsing meta.
        yr_model_run_at: None,
        resolution,
    })
}

/// Parse an HTTP date string (e.g. "Sat, 14 Feb 2026 12:00:00 GMT") into a
/// `DateTime<Utc>`. Falls back to `Utc::now() + 1 hour` if parsing fails.
pub fn parse_expires_header(expires: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc2822(expires)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Try alternative format "Sat, 14 Feb 2026 12:00:00 GMT" (httpdate)
            httpdate_parse(expires)
        })
        .unwrap_or_else(|_| {
            tracing::warn!(
                "Failed to parse Expires header '{}', defaulting to now + 1h",
                expires
            );
            Utc::now() + chrono::Duration::hours(1)
        })
}

/// Parse HTTP-date format used by Expires header.
fn httpdate_parse(s: &str) -> Result<DateTime<Utc>, String> {
    // Common HTTP date formats:
    // "Sun, 06 Nov 1994 08:49:37 GMT"     (preferred)
    // "Sunday, 06-Nov-94 08:49:37 GMT"     (obsolete RFC 850)
    // "Sun Nov  6 08:49:37 1994"           (ANSI C asctime)

    let formats = [
        "%a, %d %b %Y %H:%M:%S GMT",
        "%A, %d-%b-%y %H:%M:%S GMT",
        "%a %b %e %H:%M:%S %Y",
    ];

    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
    }

    Err(format!("Could not parse HTTP date: {}", s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Test-only convenience wrapper: extract a forecast for a single time.
    fn extract_forecast_at_time(
        raw_json: &serde_json::Value,
        forecast_time: DateTime<Utc>,
    ) -> Result<Option<YrParsedForecast>, AppError> {
        let result = extract_forecasts_at_times(raw_json.clone(), &[forecast_time])?;
        Ok(result.forecasts.into_iter().next().flatten())
    }

    #[test]
    fn test_f64_to_decimal() {
        let d = f64_to_decimal(-4.7);
        assert_eq!(d, Decimal::from_str("-4.7").unwrap());
    }

    #[test]
    fn test_f64_to_decimal_nan() {
        let d = f64_to_decimal(f64::NAN);
        assert_eq!(d, Decimal::ZERO, "NaN should be converted to 0");
    }

    #[test]
    fn test_f64_to_decimal_infinity() {
        let d = f64_to_decimal(f64::INFINITY);
        assert_eq!(d, Decimal::ZERO, "Infinity should be converted to 0");
    }

    #[test]
    fn test_f64_to_decimal_neg_infinity() {
        let d = f64_to_decimal(f64::NEG_INFINITY);
        assert_eq!(
            d,
            Decimal::ZERO,
            "Negative infinity should be converted to 0"
        );
    }

    #[test]
    fn test_opt_f64_to_decimal_some() {
        let d = opt_f64_to_decimal(Some(3.2));
        assert_eq!(d, Some(Decimal::from_str("3.2").unwrap()));
    }

    #[test]
    fn test_opt_f64_to_decimal_none() {
        let d = opt_f64_to_decimal(None);
        assert_eq!(d, None);
    }

    #[test]
    fn test_parse_expires_header_rfc2822() {
        let dt = parse_expires_header("Sat, 14 Feb 2026 12:00:00 +0000");
        assert_eq!(dt.timestamp(), 1771070400);
    }

    #[test]
    fn test_parse_expires_header_http_date() {
        let dt = parse_expires_header("Sat, 14 Feb 2026 12:00:00 GMT");
        assert_eq!(dt.timestamp(), 1771070400);
    }

    #[test]
    fn test_parse_expires_header_fallback() {
        // Invalid date should fall back to approximately now + 1h
        let dt = parse_expires_header("not-a-date");
        let now = Utc::now();
        assert!(dt > now, "Fallback should be in the future");
        assert!(
            dt < now + chrono::Duration::hours(2),
            "Fallback should be roughly now + 1h"
        );
    }

    #[test]
    fn test_extract_forecast_at_time() {
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [
                    {
                        "time": "2026-03-01T07:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -5.0,
                                    "wind_speed": 3.2,
                                    "wind_from_direction": 180.0,
                                    "relative_humidity": 75.0,
                                    "dew_point_temperature": -8.5,
                                    "cloud_area_fraction": 50.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "cloudy" },
                                "details": {
                                    "precipitation_amount": 0.0
                                }
                            }
                        }
                    },
                    {
                        "time": "2026-03-01T08:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -3.0,
                                    "wind_speed": 4.0,
                                    "wind_from_direction": 200.0,
                                    "relative_humidity": 70.0,
                                    "dew_point_temperature": -7.0,
                                    "cloud_area_fraction": 60.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "lightsnow" },
                                "details": {
                                    "precipitation_amount": 0.5
                                }
                            }
                        }
                    }
                ]
            }
        });

        let ft = "2026-03-01T07:30:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap();
        // Should pick the closest entry (07:00 is 30 min away, 08:00 is 30 min away,
        // min_by_key picks the first in case of tie = 07:00)
        // Both entries are hourly and within 1h tolerance, so result should be Some
        let forecast = result.expect("Should return Some for in-range hourly entry");
        assert_eq!(forecast.temperature_c, Decimal::from_str("-5.0").unwrap());
        // No meta block — yr_model_run_at should be None
        assert_eq!(forecast.yr_model_run_at, None);
        assert_eq!(forecast.resolution, ForecastResolution::Hourly);
    }

    #[test]
    fn test_extract_forecast_with_meta_updated_at() {
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "meta": {
                    "updated_at": "2026-02-28T14:23:45Z"
                },
                "timeseries": [
                    {
                        "time": "2026-03-01T07:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -5.0,
                                    "wind_speed": 3.2,
                                    "wind_from_direction": 180.0,
                                    "relative_humidity": 75.0,
                                    "dew_point_temperature": -8.5,
                                    "cloud_area_fraction": 50.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "cloudy" },
                                "details": { "precipitation_amount": 0.0 }
                            }
                        }
                    }
                ]
            }
        });

        let ft = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap();
        let forecast = result.expect("Should return Some for exact-match entry");
        assert_eq!(forecast.temperature_c, Decimal::from_str("-5.0").unwrap());
        let expected_model_run = "2026-02-28T14:23:45Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(forecast.yr_model_run_at, Some(expected_model_run));
    }

    #[test]
    fn test_extract_forecasts_at_times() {
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [
                    {
                        "time": "2026-03-01T07:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -5.0,
                                    "wind_speed": 3.2,
                                    "wind_from_direction": 180.0,
                                    "relative_humidity": 75.0,
                                    "dew_point_temperature": -8.5,
                                    "cloud_area_fraction": 50.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "cloudy" },
                                "details": { "precipitation_amount": 0.0 }
                            }
                        }
                    },
                    {
                        "time": "2026-03-01T10:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -2.0,
                                    "wind_speed": 5.0,
                                    "wind_from_direction": 220.0,
                                    "relative_humidity": 65.0,
                                    "dew_point_temperature": -6.0,
                                    "cloud_area_fraction": 80.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "snow" },
                                "details": { "precipitation_amount": 1.5 }
                            }
                        }
                    }
                ]
            }
        });

        let times = vec![
            "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap(),
            "2026-03-01T10:00:00Z".parse::<DateTime<Utc>>().unwrap(),
        ];

        let result = extract_forecasts_at_times(json, &times).unwrap();
        assert_eq!(result.forecasts.len(), 2);
        let f0 = result.forecasts[0]
            .as_ref()
            .expect("First entry should be Some");
        let f1 = result.forecasts[1]
            .as_ref()
            .expect("Second entry should be Some");
        assert_eq!(f0.temperature_c, Decimal::from_str("-5.0").unwrap());
        assert_eq!(f1.temperature_c, Decimal::from_str("-2.0").unwrap());
        // Horizon should be the last timeseries entry
        assert_eq!(
            result.forecast_horizon,
            "2026-03-01T10:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn test_forecast_time_field_on_extract_at_time() {
        // Verify that extract_forecast_at_time returns the yr.no native timestamp,
        // not the requested time.
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [
                    {
                        "time": "2026-03-01T07:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -5.0,
                                    "wind_speed": 3.2,
                                    "wind_from_direction": 180.0,
                                    "relative_humidity": 75.0,
                                    "dew_point_temperature": -8.5,
                                    "cloud_area_fraction": 50.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "cloudy" },
                                "details": { "precipitation_amount": 0.0 }
                            }
                        }
                    }
                ]
            }
        });

        // Request 07:14 — should snap to 07:00
        let ft = "2026-03-01T07:14:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap();
        let forecast = result.expect("Should return Some for in-range entry");
        let expected_native = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(forecast.forecast_time, expected_native);
    }

    // --- Resolution-aware tolerance tests ---

    #[test]
    fn test_resolution_detection_hourly() {
        // Entry with next_1_hours → Hourly resolution
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [{
                    "time": "2026-03-01T07:00:00Z",
                    "data": {
                        "instant": {
                            "details": {
                                "air_temperature": -5.0,
                                "wind_speed": 3.2,
                                "wind_from_direction": 180.0,
                                "relative_humidity": 75.0,
                                "dew_point_temperature": -8.5,
                                "cloud_area_fraction": 50.0
                            }
                        },
                        "next_1_hours": {
                            "summary": { "symbol_code": "cloudy" },
                            "details": { "precipitation_amount": 0.0 }
                        },
                        "next_6_hours": {
                            "summary": { "symbol_code": "cloudy" },
                            "details": { "precipitation_amount": 0.0 }
                        }
                    }
                }]
            }
        });

        let ft = "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap().unwrap();
        assert_eq!(result.resolution, ForecastResolution::Hourly);
    }

    #[test]
    fn test_resolution_detection_six_hourly() {
        // Entry with only next_6_hours (no next_1_hours) → SixHourly resolution
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [{
                    "time": "2026-03-05T12:00:00Z",
                    "data": {
                        "instant": {
                            "details": {
                                "air_temperature": -2.0,
                                "wind_speed": 4.0,
                                "wind_from_direction": 220.0,
                                "relative_humidity": 70.0,
                                "dew_point_temperature": -6.0,
                                "cloud_area_fraction": 60.0
                            }
                        },
                        "next_6_hours": {
                            "summary": { "symbol_code": "partlycloudy_day" },
                            "details": { "precipitation_amount": 0.3 }
                        }
                    }
                }]
            }
        });

        let ft = "2026-03-05T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap().unwrap();
        assert_eq!(result.resolution, ForecastResolution::SixHourly);
    }

    #[test]
    fn test_hourly_tolerance_just_within() {
        // Hourly entry at 07:00, requesting 07:59 (59 min away < 1h tolerance)
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [{
                    "time": "2026-03-01T07:00:00Z",
                    "data": {
                        "instant": {
                            "details": {
                                "air_temperature": -5.0,
                                "wind_speed": 3.2,
                                "wind_from_direction": 180.0,
                                "relative_humidity": 75.0,
                                "dew_point_temperature": -8.5,
                                "cloud_area_fraction": 50.0
                            }
                        },
                        "next_1_hours": {
                            "summary": { "symbol_code": "cloudy" },
                            "details": { "precipitation_amount": 0.0 }
                        }
                    }
                }]
            }
        });

        // 59 minutes away — within 1h tolerance
        let ft = "2026-03-01T07:59:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap();
        assert!(
            result.is_some(),
            "59 min from hourly entry should be within tolerance"
        );
    }

    #[test]
    fn test_hourly_tolerance_just_outside() {
        // Hourly entry at 07:00, requesting 08:01 (61 min away > 1h tolerance)
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [{
                    "time": "2026-03-01T07:00:00Z",
                    "data": {
                        "instant": {
                            "details": {
                                "air_temperature": -5.0,
                                "wind_speed": 3.2,
                                "wind_from_direction": 180.0,
                                "relative_humidity": 75.0,
                                "dew_point_temperature": -8.5,
                                "cloud_area_fraction": 50.0
                            }
                        },
                        "next_1_hours": {
                            "summary": { "symbol_code": "cloudy" },
                            "details": { "precipitation_amount": 0.0 }
                        }
                    }
                }]
            }
        });

        // 61 minutes away — outside 1h tolerance
        let ft = "2026-03-01T08:01:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap();
        assert!(
            result.is_none(),
            "61 min from hourly entry should exceed tolerance"
        );
    }

    #[test]
    fn test_six_hourly_tolerance_within() {
        // 6-hourly entry at 12:00, requesting 14:59 (2h59m away < 3h tolerance)
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [{
                    "time": "2026-03-05T12:00:00Z",
                    "data": {
                        "instant": {
                            "details": {
                                "air_temperature": -2.0,
                                "wind_speed": 4.0,
                                "wind_from_direction": 220.0,
                                "relative_humidity": 70.0,
                                "dew_point_temperature": -6.0,
                                "cloud_area_fraction": 60.0
                            }
                        },
                        "next_6_hours": {
                            "summary": { "symbol_code": "partlycloudy_day" },
                            "details": { "precipitation_amount": 0.3 }
                        }
                    }
                }]
            }
        });

        // 2h59m away — within 3h tolerance for 6-hourly
        let ft = "2026-03-05T14:59:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap();
        assert!(
            result.is_some(),
            "2h59m from 6-hourly entry should be within tolerance"
        );
    }

    #[test]
    fn test_six_hourly_tolerance_outside() {
        // 6-hourly entry at 12:00, requesting 15:01 (3h01m away > 3h tolerance)
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [{
                    "time": "2026-03-05T12:00:00Z",
                    "data": {
                        "instant": {
                            "details": {
                                "air_temperature": -2.0,
                                "wind_speed": 4.0,
                                "wind_from_direction": 220.0,
                                "relative_humidity": 70.0,
                                "dew_point_temperature": -6.0,
                                "cloud_area_fraction": 60.0
                            }
                        },
                        "next_6_hours": {
                            "summary": { "symbol_code": "partlycloudy_day" },
                            "details": { "precipitation_amount": 0.3 }
                        }
                    }
                }]
            }
        });

        // 3h01m away — outside 3h tolerance for 6-hourly
        let ft = "2026-03-05T15:01:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap();
        assert!(
            result.is_none(),
            "3h01m from 6-hourly entry should exceed tolerance"
        );
    }

    #[test]
    fn test_out_of_range_returns_none() {
        // yr.no has data up to March 10, requesting April 1 — way beyond horizon
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [
                    {
                        "time": "2026-03-01T07:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -5.0,
                                    "wind_speed": 3.2,
                                    "wind_from_direction": 180.0,
                                    "relative_humidity": 75.0,
                                    "dew_point_temperature": -8.5,
                                    "cloud_area_fraction": 50.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "cloudy" },
                                "details": { "precipitation_amount": 0.0 }
                            }
                        }
                    },
                    {
                        "time": "2026-03-10T18:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": 1.0,
                                    "wind_speed": 2.0,
                                    "wind_from_direction": 90.0,
                                    "relative_humidity": 60.0,
                                    "dew_point_temperature": -3.0,
                                    "cloud_area_fraction": 40.0
                                }
                            },
                            "next_6_hours": {
                                "summary": { "symbol_code": "fair_day" },
                                "details": { "precipitation_amount": 0.0 }
                            }
                        }
                    }
                ]
            }
        });

        // Request April 1 — closest yr.no entry is March 10 (~22 days away)
        let ft = "2026-04-01T08:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let result = extract_forecast_at_time(&json, ft).unwrap();
        assert!(
            result.is_none(),
            "Target far beyond yr.no horizon should return None"
        );
    }

    #[test]
    fn test_batch_mixed_in_range_and_out_of_range() {
        // Two yr.no entries (hourly). Request three times: one exact, one close, one far out.
        let json = serde_json::json!({
            "type": "Feature",
            "properties": {
                "timeseries": [
                    {
                        "time": "2026-03-01T07:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -5.0,
                                    "wind_speed": 3.2,
                                    "wind_from_direction": 180.0,
                                    "relative_humidity": 75.0,
                                    "dew_point_temperature": -8.5,
                                    "cloud_area_fraction": 50.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "cloudy" },
                                "details": { "precipitation_amount": 0.0 }
                            }
                        }
                    },
                    {
                        "time": "2026-03-01T08:00:00Z",
                        "data": {
                            "instant": {
                                "details": {
                                    "air_temperature": -3.0,
                                    "wind_speed": 4.0,
                                    "wind_from_direction": 200.0,
                                    "relative_humidity": 70.0,
                                    "dew_point_temperature": -7.0,
                                    "cloud_area_fraction": 60.0
                                }
                            },
                            "next_1_hours": {
                                "summary": { "symbol_code": "lightsnow" },
                                "details": { "precipitation_amount": 0.5 }
                            }
                        }
                    }
                ]
            }
        });

        let times = vec![
            "2026-03-01T07:00:00Z".parse::<DateTime<Utc>>().unwrap(), // exact match → Some
            "2026-03-01T07:30:00Z".parse::<DateTime<Utc>>().unwrap(), // 30m from 07:00 → Some
            "2026-04-01T12:00:00Z".parse::<DateTime<Utc>>().unwrap(), // way out → None
        ];

        let result = extract_forecasts_at_times(json, &times).unwrap();
        assert_eq!(result.forecasts.len(), 3);
        assert!(result.forecasts[0].is_some(), "Exact match should be Some");
        assert!(
            result.forecasts[1].is_some(),
            "30min offset should be Some (within 1h)"
        );
        assert!(result.forecasts[2].is_none(), "31 days out should be None");
        // Horizon is the last timeseries entry (08:00)
        assert_eq!(
            result.forecast_horizon,
            "2026-03-01T08:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn test_resolution_max_tolerance_values() {
        assert_eq!(ForecastResolution::Hourly.max_tolerance_secs(), 3_600);
        assert_eq!(ForecastResolution::SixHourly.max_tolerance_secs(), 10_800);
    }
}
