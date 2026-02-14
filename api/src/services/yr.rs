//! yr.no Locationforecast 2.0 client.
//!
//! Fetches weather forecasts from the MET Norway API.
//! See: https://api.met.no/weatherapi/locationforecast/2.0/documentation

use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, IF_MODIFIED_SINCE, USER_AGENT};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;

use crate::errors::AppError;

const YR_API_URL: &str = "https://api.met.no/weatherapi/locationforecast/2.0/complete";

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
    NotModified,
}

/// Parsed forecast data from yr.no for a specific time.
/// Extracted from a cached timeseries, not fetched directly.
#[derive(Debug, Clone)]
pub struct YrParsedForecast {
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
}

// --- yr.no JSON response types ---

#[derive(Debug, Deserialize)]
struct YrResponse {
    properties: YrProperties,
}

#[derive(Debug, Deserialize)]
struct YrProperties {
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
    Decimal::from_str(&format!("{:.1}", v)).unwrap_or_default()
}

fn opt_f64_to_decimal(v: Option<f64>) -> Option<Decimal> {
    v.map(f64_to_decimal)
}

impl YrClient {
    pub fn new(user_agent: &str) -> Self {
        let client = reqwest::Client::builder()
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

        // Handle 304 Not Modified
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(YrTimeseriesResult::NotModified);
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

/// Extract a forecast for a specific time from a cached yr.no timeseries JSON.
///
/// This is a pure function (no I/O) — it finds the closest timeseries entry
/// to `forecast_time` and converts it to `YrParsedForecast`.
///
/// Delegates to `extract_forecasts_at_times` for a single time to avoid
/// duplicating the find-closest + parse logic.
pub fn extract_forecast_at_time(
    raw_json: &serde_json::Value,
    forecast_time: DateTime<Utc>,
) -> Result<YrParsedForecast, AppError> {
    let mut results = extract_forecasts_at_times(raw_json, &[forecast_time])?;
    Ok(results.remove(0))
}

/// Extract forecasts for multiple times from a single cached yr.no timeseries.
///
/// Much more efficient than calling `extract_forecast_at_time` N times because
/// we deserialize the JSON only once.
pub fn extract_forecasts_at_times(
    raw_json: &serde_json::Value,
    forecast_times: &[DateTime<Utc>],
) -> Result<Vec<YrParsedForecast>, AppError> {
    let yr_response: YrResponse = serde_json::from_value(raw_json.clone()).map_err(|e| {
        AppError::ExternalServiceError(format!("yr.no response structure error: {}", e))
    })?;

    let timeseries = &yr_response.properties.timeseries;
    if timeseries.is_empty() {
        return Err(AppError::ExternalServiceError(
            "yr.no returned empty timeseries".to_string(),
        ));
    }

    let mut results = Vec::with_capacity(forecast_times.len());

    for &ft in forecast_times {
        let target_ts = ft.timestamp();
        let closest = timeseries
            .iter()
            .min_by_key(|ts| {
                let ts_time = chrono::DateTime::parse_from_rfc3339(&ts.time)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);
                (ts_time - target_ts).unsigned_abs()
            })
            .ok_or_else(|| {
                AppError::ExternalServiceError("yr.no returned empty timeseries".to_string())
            })?;

        results.push(parse_timeseries_entry(closest)?);
    }

    Ok(results)
}

/// Parse a single yr.no timeseries entry into a `YrParsedForecast`.
fn parse_timeseries_entry(entry: &YrTimeseries) -> Result<YrParsedForecast, AppError> {
    let instant = &entry.data.instant.details;

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

    Ok(YrParsedForecast {
        temperature_c: f64_to_decimal(instant.air_temperature.unwrap_or(0.0)),
        temperature_percentile_10_c: opt_f64_to_decimal(instant.air_temperature_percentile_10),
        temperature_percentile_90_c: opt_f64_to_decimal(instant.air_temperature_percentile_90),
        wind_speed_ms: f64_to_decimal(instant.wind_speed.unwrap_or(0.0)),
        wind_speed_percentile_10_ms: opt_f64_to_decimal(instant.wind_speed_percentile_10),
        wind_speed_percentile_90_ms: opt_f64_to_decimal(instant.wind_speed_percentile_90),
        wind_direction_deg: f64_to_decimal(instant.wind_from_direction.unwrap_or(0.0)),
        wind_gust_ms: opt_f64_to_decimal(instant.wind_speed_of_gust),
        precipitation_mm: f64_to_decimal(
            precip.and_then(|p| p.precipitation_amount).unwrap_or(0.0),
        ),
        precipitation_min_mm: opt_f64_to_decimal(precip.and_then(|p| p.precipitation_amount_min)),
        precipitation_max_mm: opt_f64_to_decimal(precip.and_then(|p| p.precipitation_amount_max)),
        humidity_pct: f64_to_decimal(instant.relative_humidity.unwrap_or(0.0)),
        dew_point_c: f64_to_decimal(instant.dew_point_temperature.unwrap_or(0.0)),
        cloud_cover_pct: f64_to_decimal(instant.cloud_area_fraction.unwrap_or(0.0)),
        uv_index: opt_f64_to_decimal(instant.ultraviolet_index_clear_sky),
        symbol_code,
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

    #[test]
    fn test_f64_to_decimal() {
        let d = f64_to_decimal(-4.7);
        assert_eq!(d, Decimal::from_str("-4.7").unwrap());
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
        assert_eq!(result.temperature_c, Decimal::from_str("-5.0").unwrap());
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

        let results = extract_forecasts_at_times(&json, &times).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].temperature_c, Decimal::from_str("-5.0").unwrap());
        assert_eq!(results[1].temperature_c, Decimal::from_str("-2.0").unwrap());
    }
}
