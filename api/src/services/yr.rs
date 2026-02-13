//! yr.no Locationforecast 2.0 client.
//!
//! Fetches weather forecasts from the MET Norway API.
//! See: https://api.met.no/weatherapi/locationforecast/2.0/documentation

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

/// The result of a yr.no API call.
pub enum YrFetchResult {
    /// New forecast data received (HTTP 200).
    NewData(Box<YrParsedForecast>),
    /// Data not modified since last fetch (HTTP 304).
    NotModified,
}

/// Parsed forecast data from yr.no for a specific time.
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
    pub raw_response: serde_json::Value,
    /// The `Expires` header value for caching (reserved for future use).
    #[allow(dead_code)]
    pub expires: Option<String>,
    /// The `Last-Modified` header value for conditional requests (reserved for future use).
    #[allow(dead_code)]
    pub last_modified: Option<String>,
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

    /// Fetch forecast from yr.no for a given location.
    ///
    /// `if_modified_since` — if provided, sends conditional request.
    /// `forecast_time` — the time we want the forecast FOR. We pick the closest
    /// timeseries entry from the yr.no response.
    pub async fn fetch_forecast(
        &self,
        lat: f64,
        lon: f64,
        altitude: f64,
        forecast_time: chrono::DateTime<chrono::Utc>,
        if_modified_since: Option<&str>,
    ) -> Result<YrFetchResult, AppError> {
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
            return Ok(YrFetchResult::NotModified);
        }

        if !response.status().is_success() {
            return Err(AppError::ExternalServiceError(format!(
                "yr.no returned HTTP {}",
                response.status()
            )));
        }

        // Extract caching headers
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

        let raw: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError(format!("yr.no JSON parse error: {}", e))
        })?;

        // Parse into typed struct
        let yr_response: YrResponse = serde_json::from_value(raw.clone()).map_err(|e| {
            AppError::ExternalServiceError(format!("yr.no response structure error: {}", e))
        })?;

        // Find the timeseries entry closest to the requested forecast_time
        let target_ts = forecast_time.timestamp();
        let closest = yr_response
            .properties
            .timeseries
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

        let instant = &closest.data.instant.details;

        // Get period data (prefer next_1_hours, fall back to next_6_hours)
        let period = closest
            .data
            .next_1_hours
            .as_ref()
            .or(closest.data.next_6_hours.as_ref());

        let symbol_code = period
            .and_then(|p| p.summary.as_ref())
            .and_then(|s| s.symbol_code.as_ref())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let precip = period.and_then(|p| p.details.as_ref());

        let parsed = YrParsedForecast {
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
            precipitation_min_mm: opt_f64_to_decimal(
                precip.and_then(|p| p.precipitation_amount_min),
            ),
            precipitation_max_mm: opt_f64_to_decimal(
                precip.and_then(|p| p.precipitation_amount_max),
            ),
            humidity_pct: f64_to_decimal(instant.relative_humidity.unwrap_or(0.0)),
            dew_point_c: f64_to_decimal(instant.dew_point_temperature.unwrap_or(0.0)),
            cloud_cover_pct: f64_to_decimal(instant.cloud_area_fraction.unwrap_or(0.0)),
            uv_index: opt_f64_to_decimal(instant.ultraviolet_index_clear_sky),
            symbol_code,
            raw_response: raw,
            expires,
            last_modified,
        };

        Ok(YrFetchResult::NewData(Box::new(parsed)))
    }
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
}
