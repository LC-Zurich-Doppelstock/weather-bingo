use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use uuid::Uuid;

/// Cached yr.no full timeseries response, keyed by checkpoint.
/// Uses yr.no's Expires/Last-Modified headers for cache validity.
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)] // All fields populated by FromRow; some accessed only via route serialization
pub struct YrCachedResponse {
    pub id: Uuid,
    pub checkpoint_id: Uuid,
    pub latitude: Decimal,
    pub longitude: Decimal,
    pub elevation_m: Decimal,
    pub fetched_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_modified: Option<String>,
    pub raw_response: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Race summary (without GPX data), used for list and summary endpoints.
#[derive(Debug, Clone, FromRow)]
pub struct Race {
    pub id: Uuid,
    pub name: String,
    pub year: i32,
    pub start_time: DateTime<Utc>,
    pub distance_km: Decimal,
}

/// A checkpoint along a race course.
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)] // All fields populated by FromRow; some accessed only via route serialization
pub struct Checkpoint {
    pub id: Uuid,
    pub race_id: Uuid,
    pub name: String,
    pub distance_km: Decimal,
    pub latitude: Decimal,
    pub longitude: Decimal,
    pub elevation_m: Decimal,
    pub sort_order: i32,
}

/// A weather forecast record for a checkpoint at a specific time.
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)] // All fields populated by FromRow; some accessed only via route serialization
pub struct Forecast {
    pub id: Uuid,
    pub checkpoint_id: Uuid,
    pub forecast_time: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
    pub source: String,

    // Weather parameters from yr.no
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

    // Calculated by API
    pub feels_like_c: Decimal,
    pub precipitation_type: String,
    /// Estimated snow surface temperature in °C (calculated from air temp, cloud cover, wind).
    /// NULL for rows created before this column was added.
    pub snow_temperature_c: Option<Decimal>,

    /// When yr.no's weather model generated this forecast.
    /// NULL for rows created before this column was added.
    pub yr_model_run_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,
}
