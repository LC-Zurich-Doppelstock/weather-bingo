//! Forecast HTTP endpoints.
//!
//! - GET /api/v1/forecasts/checkpoint/:checkpoint_id?datetime=ISO8601
//! - GET /api/v1/forecasts/checkpoint/:checkpoint_id/history?datetime=ISO8601
//! - GET /api/v1/forecasts/race/:race_id?target_duration_hours=N

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::db::{models, queries};
use crate::errors::{AppError, ErrorResponse};
use crate::services::forecast::{
    calculate_pass_time_fractions, calculate_pass_time_weighted, get_checkpoint, resolve_forecast,
    resolve_race_forecasts, CheckpointWithTime, PacingCheckpoint,
};
use crate::services::yr::YrClient;

/// Shared application state for forecast endpoints.
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub yr_client: YrClient,
}

// ---------------------------------------------------------------------------
// Query parameter structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, IntoParams)]
pub struct ForecastQuery {
    /// Target datetime in ISO 8601 format (e.g. "2026-03-01T08:00:00Z")
    pub datetime: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct RaceForecastQuery {
    /// Target race duration in hours (e.g. 8.0 for an 8-hour finish)
    pub target_duration_hours: f64,
}

// ---------------------------------------------------------------------------
// Response types — Section 9.4
// ---------------------------------------------------------------------------

/// Unified weather data for both checkpoint detail and race overview.
///
/// All core fields are always present. Detail-only fields (wind gusts,
/// precipitation range, humidity, dew point, cloud cover, UV) are `Option`
/// and omitted from JSON when `None` via `skip_serializing_if`.
///
/// - `Weather::full()` — sets all fields (checkpoint detail view)
/// - `Weather::simplified()` — sets detail-only fields to `None` (race overview)
#[derive(Debug, Serialize, ToSchema)]
pub struct Weather {
    /// Air temperature in Celsius
    pub temperature_c: f64,
    /// 10th percentile temperature (uncertainty low bound)
    pub temperature_percentile_10_c: Option<f64>,
    /// 90th percentile temperature (uncertainty high bound)
    pub temperature_percentile_90_c: Option<f64>,
    /// Feels-like temperature (wind chill adjusted) in Celsius
    pub feels_like_c: f64,
    /// Estimated snow surface temperature in Celsius (for wax selection)
    pub snow_temperature_c: f64,
    /// Wind speed in metres per second
    pub wind_speed_ms: f64,
    /// 10th percentile wind speed
    pub wind_speed_percentile_10_ms: Option<f64>,
    /// 90th percentile wind speed
    pub wind_speed_percentile_90_ms: Option<f64>,
    /// Wind direction in degrees (0 = north, 90 = east)
    pub wind_direction_deg: f64,
    /// Wind gust speed in m/s (detail view only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wind_gust_ms: Option<f64>,
    /// Precipitation amount in mm/h
    pub precipitation_mm: f64,
    /// Minimum expected precipitation in mm/h (detail view only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precipitation_min_mm: Option<f64>,
    /// Maximum expected precipitation in mm/h (detail view only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precipitation_max_mm: Option<f64>,
    /// Precipitation type: "snow", "rain", "sleet", or "none"
    pub precipitation_type: String,
    /// Relative humidity percentage (detail view only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub humidity_pct: Option<f64>,
    /// Dew point temperature in Celsius (detail view only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dew_point_c: Option<f64>,
    /// Cloud cover percentage (detail view only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_cover_pct: Option<f64>,
    /// UV index (detail view only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uv_index: Option<f64>,
    /// yr.no weather symbol code (e.g. "cloudy", "lightssnowshowers_day")
    pub symbol_code: String,
}

impl Weather {
    /// Full weather from a forecast (checkpoint detail view).
    /// All fields populated — detail-only fields are `Some(value)`.
    pub fn full(f: &models::Forecast) -> Self {
        Self {
            temperature_c: f.temperature_c.to_f64().unwrap_or(0.0),
            temperature_percentile_10_c: f.temperature_percentile_10_c.and_then(|v| v.to_f64()),
            temperature_percentile_90_c: f.temperature_percentile_90_c.and_then(|v| v.to_f64()),
            feels_like_c: f.feels_like_c.to_f64().unwrap_or(0.0),
            snow_temperature_c: f.snow_temperature_c.and_then(|v| v.to_f64()).unwrap_or(0.0),
            wind_speed_ms: f.wind_speed_ms.to_f64().unwrap_or(0.0),
            wind_speed_percentile_10_ms: f.wind_speed_percentile_10_ms.and_then(|v| v.to_f64()),
            wind_speed_percentile_90_ms: f.wind_speed_percentile_90_ms.and_then(|v| v.to_f64()),
            wind_direction_deg: f.wind_direction_deg.to_f64().unwrap_or(0.0),
            wind_gust_ms: f.wind_gust_ms.and_then(|v| v.to_f64()),
            precipitation_mm: f.precipitation_mm.to_f64().unwrap_or(0.0),
            precipitation_min_mm: f.precipitation_min_mm.and_then(|v| v.to_f64()),
            precipitation_max_mm: f.precipitation_max_mm.and_then(|v| v.to_f64()),
            precipitation_type: f.precipitation_type.clone(),
            humidity_pct: Some(f.humidity_pct.to_f64().unwrap_or(0.0)),
            dew_point_c: Some(f.dew_point_c.to_f64().unwrap_or(0.0)),
            cloud_cover_pct: Some(f.cloud_cover_pct.to_f64().unwrap_or(0.0)),
            uv_index: f.uv_index.and_then(|v| v.to_f64()),
            symbol_code: f.symbol_code.clone(),
        }
    }

    /// Simplified weather for race overview (omits detail-only fields).
    /// Detail-only fields are `None` and will be omitted from JSON.
    pub fn simplified(f: &models::Forecast) -> Self {
        Self {
            temperature_c: f.temperature_c.to_f64().unwrap_or(0.0),
            temperature_percentile_10_c: f.temperature_percentile_10_c.and_then(|v| v.to_f64()),
            temperature_percentile_90_c: f.temperature_percentile_90_c.and_then(|v| v.to_f64()),
            feels_like_c: f.feels_like_c.to_f64().unwrap_or(0.0),
            snow_temperature_c: f.snow_temperature_c.and_then(|v| v.to_f64()).unwrap_or(0.0),
            wind_speed_ms: f.wind_speed_ms.to_f64().unwrap_or(0.0),
            wind_speed_percentile_10_ms: f.wind_speed_percentile_10_ms.and_then(|v| v.to_f64()),
            wind_speed_percentile_90_ms: f.wind_speed_percentile_90_ms.and_then(|v| v.to_f64()),
            wind_direction_deg: f.wind_direction_deg.to_f64().unwrap_or(0.0),
            wind_gust_ms: None,
            precipitation_mm: f.precipitation_mm.to_f64().unwrap_or(0.0),
            precipitation_min_mm: None,
            precipitation_max_mm: None,
            precipitation_type: f.precipitation_type.clone(),
            humidity_pct: None,
            dew_point_c: None,
            cloud_cover_pct: None,
            uv_index: None,
            symbol_code: f.symbol_code.clone(),
        }
    }
}

/// Checkpoint forecast response (Section 9.4).
#[derive(Debug, Serialize, ToSchema)]
pub struct ForecastResponse {
    /// Checkpoint UUID
    pub checkpoint_id: Uuid,
    /// Checkpoint name
    pub checkpoint_name: String,
    /// The datetime the forecast is for (ISO 8601).
    /// When `forecast_available` is false, this is the originally requested time.
    pub forecast_time: String,
    /// Whether forecast data is available for the requested time.
    /// `false` when the race date is beyond yr.no's forecast horizon.
    pub forecast_available: bool,
    /// When this forecast was last fetched from the source (ISO 8601).
    /// Null when `forecast_available` is false.
    pub fetched_at: Option<String>,
    /// When yr.no's weather model generated this forecast (ISO 8601).
    /// Null for older rows that predate this tracking, or when forecast is unavailable.
    pub yr_model_run_at: Option<String>,
    /// Forecast data source (e.g. "yr.no"). Null when forecast is unavailable.
    pub source: Option<String>,
    /// Whether this forecast is stale (yr.no was unreachable, serving cached data)
    pub stale: bool,
    /// The furthest datetime yr.no currently forecasts to (ISO 8601).
    /// Null when yr.no cache is unavailable (stale fallback).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forecast_horizon: Option<String>,
    /// Full weather data. Null when `forecast_available` is false.
    pub weather: Option<Weather>,
}

/// A single historical forecast entry showing weather at a previous fetch time.
#[derive(Debug, Serialize, ToSchema)]
pub struct ForecastHistoryEntry {
    /// When this version of the forecast was fetched (ISO 8601)
    pub fetched_at: String,
    /// When yr.no's weather model generated this forecast (ISO 8601).
    /// Null for older rows that predate this tracking.
    pub yr_model_run_at: Option<String>,
    /// Weather data at this fetch time
    pub weather: Weather,
}

/// Forecast history response showing how a forecast has evolved (Section 9.5).
#[derive(Debug, Serialize, ToSchema)]
pub struct ForecastHistoryResponse {
    /// Checkpoint UUID
    pub checkpoint_id: Uuid,
    /// Checkpoint name
    pub checkpoint_name: String,
    /// The datetime the forecast is for (ISO 8601)
    pub forecast_time: String,
    /// Historical forecast entries, ordered by fetch time
    pub history: Vec<ForecastHistoryEntry>,
}

/// A checkpoint with its expected weather in the race forecast (Section 9.6).
#[derive(Debug, Serialize, ToSchema)]
pub struct RaceForecastCheckpoint {
    /// Checkpoint UUID
    pub checkpoint_id: Uuid,
    /// Checkpoint name
    pub name: String,
    /// Distance from race start in km
    pub distance_km: f64,
    /// Expected pass-through time based on elevation-adjusted pacing (ISO 8601)
    pub expected_time: String,
    /// Whether forecast data is available for this checkpoint's expected time.
    /// `false` when the race date is beyond yr.no's ~10-day forecast horizon.
    pub forecast_available: bool,
    /// Simplified weather at expected pass-through time.
    /// Null when `forecast_available` is false.
    pub weather: Option<Weather>,
}

/// Full race forecast response with weather at all checkpoints (Section 9.6).
#[derive(Debug, Serialize, ToSchema)]
pub struct RaceForecastResponse {
    /// Race UUID
    pub race_id: Uuid,
    /// Race name
    pub race_name: String,
    /// Target duration used for pacing calculation
    pub target_duration_hours: f64,
    /// When yr.no's weather model generated the forecast data (ISO 8601).
    /// Uses the oldest model run across all checkpoints, or null if unknown.
    pub yr_model_run_at: Option<String>,
    /// The furthest datetime yr.no currently forecasts to (ISO 8601).
    /// Uses the minimum horizon across all checkpoints (most conservative), or null if unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forecast_horizon: Option<String>,
    /// Weather forecasts at each checkpoint
    pub checkpoints: Vec<RaceForecastCheckpoint>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Get the latest forecast for a checkpoint at a specific datetime.
///
/// Returns the most recent forecast for the given checkpoint closest to the
/// specified datetime. If yr.no is unavailable, returns stale cached data
/// with the `X-Forecast-Stale: true` header.
#[utoipa::path(
    get,
    path = "/api/v1/forecasts/checkpoint/{checkpoint_id}",
    tag = "Forecasts",
    params(
        ("checkpoint_id" = Uuid, Path, description = "Checkpoint UUID"),
        ForecastQuery,
    ),
    responses(
        (status = 200, description = "Latest forecast for the checkpoint", body = ForecastResponse,
         headers(
             ("X-Forecast-Stale" = String, description = "Set to 'true' when serving cached data because yr.no is unreachable")
         )),
        (status = 400, description = "Invalid datetime format", body = ErrorResponse),
        (status = 404, description = "Checkpoint not found", body = ErrorResponse),
        (status = 502, description = "External service error (yr.no unreachable, no cache)", body = ErrorResponse),
    )
)]
pub async fn get_checkpoint_forecast(
    State(state): State<AppState>,
    Path(checkpoint_id): Path<Uuid>,
    Query(params): Query<ForecastQuery>,
) -> Result<(HeaderMap, Json<ForecastResponse>), AppError> {
    let forecast_time: DateTime<Utc> = params
        .datetime
        .parse()
        .map_err(|e| AppError::BadRequest(format!("Invalid datetime: {}", e)))?;

    let checkpoint = get_checkpoint(&state.pool, checkpoint_id).await?;

    let (maybe_forecast, is_stale, forecast_horizon) =
        resolve_forecast(&state.pool, &state.yr_client, &checkpoint, forecast_time).await?;

    let horizon_str = forecast_horizon.map(|dt| dt.to_rfc3339());

    let response = match maybe_forecast {
        Some(forecast) => ForecastResponse {
            checkpoint_id: checkpoint.id,
            checkpoint_name: checkpoint.name.clone(),
            forecast_time: forecast.forecast_time.to_rfc3339(),
            forecast_available: true,
            fetched_at: Some(forecast.fetched_at.to_rfc3339()),
            yr_model_run_at: forecast.yr_model_run_at.map(|dt| dt.to_rfc3339()),
            source: Some(forecast.source.clone()),
            stale: is_stale,
            forecast_horizon: horizon_str,
            weather: Some(Weather::full(&forecast)),
        },
        None => ForecastResponse {
            checkpoint_id: checkpoint.id,
            checkpoint_name: checkpoint.name.clone(),
            forecast_time: forecast_time.to_rfc3339(),
            forecast_available: false,
            fetched_at: None,
            yr_model_run_at: None,
            source: None,
            stale: false,
            forecast_horizon: horizon_str,
            weather: None,
        },
    };

    let mut headers = HeaderMap::new();
    if is_stale {
        headers.insert("X-Forecast-Stale", "true".parse().unwrap());
    }

    Ok((headers, Json(response)))
}

/// Get the forecast history for a checkpoint, showing how predictions evolved.
///
/// Returns all previously fetched forecasts for a checkpoint at the given
/// datetime, ordered by fetch time. This allows users to see how the
/// forecast has changed over days/hours leading up to the race.
#[utoipa::path(
    get,
    path = "/api/v1/forecasts/checkpoint/{checkpoint_id}/history",
    tag = "Forecasts",
    params(
        ("checkpoint_id" = Uuid, Path, description = "Checkpoint UUID"),
        ForecastQuery,
    ),
    responses(
        (status = 200, description = "Forecast history for the checkpoint", body = ForecastHistoryResponse),
        (status = 400, description = "Invalid datetime format", body = ErrorResponse),
        (status = 404, description = "Checkpoint not found", body = ErrorResponse),
    )
)]
pub async fn get_checkpoint_forecast_history(
    State(state): State<AppState>,
    Path(checkpoint_id): Path<Uuid>,
    Query(params): Query<ForecastQuery>,
) -> Result<Json<ForecastHistoryResponse>, AppError> {
    let forecast_time: DateTime<Utc> = params
        .datetime
        .parse()
        .map_err(|e| AppError::BadRequest(format!("Invalid datetime: {}", e)))?;

    let checkpoint = get_checkpoint(&state.pool, checkpoint_id).await?;

    let forecasts =
        queries::get_forecast_history(&state.pool, checkpoint_id, forecast_time).await?;

    let history: Vec<ForecastHistoryEntry> = forecasts
        .iter()
        .map(|f| ForecastHistoryEntry {
            fetched_at: f.fetched_at.to_rfc3339(),
            yr_model_run_at: f.yr_model_run_at.map(|dt| dt.to_rfc3339()),
            weather: Weather::full(f),
        })
        .collect();

    let response_time = if let Some(first) = forecasts.first() {
        first.forecast_time.to_rfc3339()
    } else {
        forecast_time.to_rfc3339()
    };

    Ok(Json(ForecastHistoryResponse {
        checkpoint_id: checkpoint.id,
        checkpoint_name: checkpoint.name,
        forecast_time: response_time,
        history,
    }))
}

/// Get weather forecasts for all checkpoints in a race.
///
/// Calculates expected pass-through times for each checkpoint using
/// elevation-adjusted pacing based on the target duration, then returns
/// the latest weather forecast for each checkpoint at its expected time.
#[utoipa::path(
    get,
    path = "/api/v1/forecasts/race/{race_id}",
    tag = "Forecasts",
    params(
        ("race_id" = Uuid, Path, description = "Race UUID"),
        RaceForecastQuery,
    ),
    responses(
        (status = 200, description = "Race forecast with weather at all checkpoints", body = RaceForecastResponse,
         headers(
             ("X-Forecast-Stale" = String, description = "Set to 'true' when serving cached data because yr.no is unreachable")
         )),
        (status = 400, description = "Invalid query parameters", body = ErrorResponse),
        (status = 404, description = "Race not found", body = ErrorResponse),
    )
)]
pub async fn get_race_forecast(
    State(state): State<AppState>,
    Path(race_id): Path<Uuid>,
    Query(params): Query<RaceForecastQuery>,
) -> Result<(HeaderMap, Json<RaceForecastResponse>), AppError> {
    // Validate target_duration_hours — check is_finite() first because NaN
    // passes range comparisons (NaN <= 0.0 is false, NaN > 72.0 is also false).
    if !params.target_duration_hours.is_finite() {
        return Err(AppError::BadRequest(
            "target_duration_hours must be a finite number".to_string(),
        ));
    }
    if params.target_duration_hours <= 0.0 || params.target_duration_hours > 72.0 {
        return Err(AppError::BadRequest(
            "target_duration_hours must be between 0 (exclusive) and 72".to_string(),
        ));
    }

    // Use lightweight query — no GPX blob
    let race = queries::get_race_summary(&state.pool, race_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Race {} not found", race_id)))?;

    let checkpoints = queries::get_checkpoints(&state.pool, race_id).await?;

    // Compute elevation-adjusted time fractions
    let pacing_inputs: Vec<PacingCheckpoint> = checkpoints
        .iter()
        .map(|cp| PacingCheckpoint {
            distance_km: cp.distance_km.to_f64().unwrap_or(0.0),
            elevation_m: cp.elevation_m.to_f64().unwrap_or(0.0),
        })
        .collect();
    let time_fractions = calculate_pass_time_fractions(&pacing_inputs);

    // Build checkpoint + expected time pairs using elevation-adjusted pacing
    let checkpoints_with_times: Vec<CheckpointWithTime> = checkpoints
        .into_iter()
        .zip(time_fractions.iter())
        .map(|(cp, &fraction)| {
            let expected_time = calculate_pass_time_weighted(
                race.start_time,
                fraction,
                params.target_duration_hours,
            );
            CheckpointWithTime {
                checkpoint: cp,
                forecast_time: expected_time,
            }
        })
        .collect();

    // Resolve all forecasts (parallel yr.no fetches per checkpoint)
    let resolved =
        resolve_race_forecasts(&state.pool, &state.yr_client, &checkpoints_with_times).await?;

    let checkpoint_forecasts: Vec<RaceForecastCheckpoint> = checkpoints_with_times
        .iter()
        .zip(resolved.iter())
        .map(|(cpwt, res)| {
            let weather = res.forecast.as_ref().map(Weather::simplified);

            RaceForecastCheckpoint {
                checkpoint_id: cpwt.checkpoint.id,
                name: cpwt.checkpoint.name.clone(),
                distance_km: cpwt.checkpoint.distance_km.to_f64().unwrap_or(0.0),
                expected_time: cpwt.forecast_time.to_rfc3339(),
                forecast_available: weather.is_some(),
                weather,
            }
        })
        .collect();

    // Find the oldest model run time across all checkpoints that have forecasts
    // (oldest = most conservative indicator of forecast freshness)
    let yr_model_run_at = resolved
        .iter()
        .filter_map(|r| r.forecast.as_ref())
        .filter_map(|f| f.yr_model_run_at)
        .min()
        .map(|dt| dt.to_rfc3339());

    // Find the minimum forecast horizon across all checkpoints (most conservative)
    let forecast_horizon = resolved
        .iter()
        .filter_map(|r| r.forecast_horizon)
        .min()
        .map(|dt| dt.to_rfc3339());

    let any_stale = resolved.iter().any(|r| r.is_stale);
    let mut headers = HeaderMap::new();
    if any_stale {
        headers.insert("X-Forecast-Stale", "true".parse().unwrap());
    }

    Ok((
        headers,
        Json(RaceForecastResponse {
            race_id: race.id,
            race_name: race.name,
            target_duration_hours: params.target_duration_hours,
            yr_model_run_at,
            forecast_horizon,
            checkpoints: checkpoint_forecasts,
        }),
    ))
}
