use rust_decimal::prelude::FromPrimitive;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{Checkpoint, Forecast, Race, RaceDetail};
use crate::services::gpx::GpxRace;

/// Parameters for inserting a new forecast record.
pub struct InsertForecastParams {
    pub checkpoint_id: Uuid,
    pub forecast_time: chrono::DateTime<chrono::Utc>,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub source: String,
    pub temperature_c: rust_decimal::Decimal,
    pub temperature_percentile_10_c: Option<rust_decimal::Decimal>,
    pub temperature_percentile_90_c: Option<rust_decimal::Decimal>,
    pub wind_speed_ms: rust_decimal::Decimal,
    pub wind_speed_percentile_10_ms: Option<rust_decimal::Decimal>,
    pub wind_speed_percentile_90_ms: Option<rust_decimal::Decimal>,
    pub wind_direction_deg: rust_decimal::Decimal,
    pub wind_gust_ms: Option<rust_decimal::Decimal>,
    pub precipitation_mm: rust_decimal::Decimal,
    pub precipitation_min_mm: Option<rust_decimal::Decimal>,
    pub precipitation_max_mm: Option<rust_decimal::Decimal>,
    pub humidity_pct: rust_decimal::Decimal,
    pub dew_point_c: rust_decimal::Decimal,
    pub cloud_cover_pct: rust_decimal::Decimal,
    pub uv_index: Option<rust_decimal::Decimal>,
    pub symbol_code: String,
    pub feels_like_c: rust_decimal::Decimal,
    pub precipitation_type: String,
    pub raw_response: serde_json::Value,
}

/// List all races (summary only, no GPX).
pub async fn list_races(pool: &PgPool) -> Result<Vec<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(
        "SELECT id, name, year, start_time, distance_km FROM races ORDER BY year DESC, name",
    )
    .fetch_all(pool)
    .await
}

/// Get a single race by ID (includes GPX).
pub async fn get_race(pool: &PgPool, id: Uuid) -> Result<Option<RaceDetail>, sqlx::Error> {
    sqlx::query_as::<_, RaceDetail>(
        "SELECT id, name, year, start_time, distance_km, course_gpx, created_at, updated_at
         FROM races WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Get all checkpoints for a race, ordered by sort_order.
pub async fn get_checkpoints(pool: &PgPool, race_id: Uuid) -> Result<Vec<Checkpoint>, sqlx::Error> {
    sqlx::query_as::<_, Checkpoint>(
        "SELECT id, race_id, name, distance_km, latitude, longitude, elevation_m, sort_order
         FROM checkpoints
         WHERE race_id = $1
         ORDER BY sort_order",
    )
    .bind(race_id)
    .fetch_all(pool)
    .await
}

/// Get the latest forecast for a checkpoint closest to a given forecast time.
pub async fn get_latest_forecast(
    pool: &PgPool,
    checkpoint_id: Uuid,
    forecast_time: chrono::DateTime<chrono::Utc>,
) -> Result<Option<Forecast>, sqlx::Error> {
    sqlx::query_as::<_, Forecast>(
        "SELECT id, checkpoint_id, forecast_time, fetched_at, source,
                temperature_c, temperature_percentile_10_c, temperature_percentile_90_c,
                wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms,
                wind_direction_deg, wind_gust_ms,
                precipitation_mm, precipitation_min_mm, precipitation_max_mm,
                humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code,
                feels_like_c, precipitation_type, created_at
         FROM forecasts
         WHERE checkpoint_id = $1
         ORDER BY ABS(EXTRACT(EPOCH FROM (forecast_time - $2))), fetched_at DESC
         LIMIT 1",
    )
    .bind(checkpoint_id)
    .bind(forecast_time)
    .fetch_optional(pool)
    .await
}

/// Get forecast history for a checkpoint at a specific forecast time.
/// Returns all fetched versions, ordered by fetched_at ascending.
pub async fn get_forecast_history(
    pool: &PgPool,
    checkpoint_id: Uuid,
    forecast_time: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<Forecast>, sqlx::Error> {
    sqlx::query_as::<_, Forecast>(
        "SELECT id, checkpoint_id, forecast_time, fetched_at, source,
                temperature_c, temperature_percentile_10_c, temperature_percentile_90_c,
                wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms,
                wind_direction_deg, wind_gust_ms,
                precipitation_mm, precipitation_min_mm, precipitation_max_mm,
                humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code,
                feels_like_c, precipitation_type, created_at
         FROM forecasts
         WHERE checkpoint_id = $1
           AND forecast_time = (
               SELECT forecast_time FROM forecasts
               WHERE checkpoint_id = $1
               ORDER BY ABS(EXTRACT(EPOCH FROM (forecast_time - $2)))
               LIMIT 1
           )
         ORDER BY fetched_at ASC",
    )
    .bind(checkpoint_id)
    .bind(forecast_time)
    .fetch_all(pool)
    .await
}

/// Insert a new forecast record (append-only).
pub async fn insert_forecast(
    pool: &PgPool,
    params: InsertForecastParams,
) -> Result<Forecast, sqlx::Error> {
    sqlx::query_as::<_, Forecast>(
        "INSERT INTO forecasts (
            id, checkpoint_id, forecast_time, fetched_at, source,
            temperature_c, temperature_percentile_10_c, temperature_percentile_90_c,
            wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms,
            wind_direction_deg, wind_gust_ms,
            precipitation_mm, precipitation_min_mm, precipitation_max_mm,
            humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code,
            feels_like_c, precipitation_type, raw_response, created_at
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8, $9, $10, $11, $12, $13,
            $14, $15, $16, $17, $18, $19, $20, $21,
            $22, $23, $24, NOW()
        )
        RETURNING id, checkpoint_id, forecast_time, fetched_at, source,
                  temperature_c, temperature_percentile_10_c, temperature_percentile_90_c,
                  wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms,
                  wind_direction_deg, wind_gust_ms,
                  precipitation_mm, precipitation_min_mm, precipitation_max_mm,
                  humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code,
                  feels_like_c, precipitation_type, created_at",
    )
    .bind(Uuid::new_v4())
    .bind(params.checkpoint_id)
    .bind(params.forecast_time)
    .bind(params.fetched_at)
    .bind(&params.source)
    .bind(params.temperature_c)
    .bind(params.temperature_percentile_10_c)
    .bind(params.temperature_percentile_90_c)
    .bind(params.wind_speed_ms)
    .bind(params.wind_speed_percentile_10_ms)
    .bind(params.wind_speed_percentile_90_ms)
    .bind(params.wind_direction_deg)
    .bind(params.wind_gust_ms)
    .bind(params.precipitation_mm)
    .bind(params.precipitation_min_mm)
    .bind(params.precipitation_max_mm)
    .bind(params.humidity_pct)
    .bind(params.dew_point_c)
    .bind(params.cloud_cover_pct)
    .bind(params.uv_index)
    .bind(&params.symbol_code)
    .bind(params.feels_like_c)
    .bind(&params.precipitation_type)
    .bind(params.raw_response)
    .fetch_one(pool)
    .await
}

/// Upsert a race and its checkpoints from parsed GPX data.
///
/// Uses INSERT ON CONFLICT (name, year) for the race, and
/// INSERT ON CONFLICT (race_id, sort_order) for each checkpoint.
/// Returns the race UUID (existing or newly created).
pub async fn upsert_race_from_gpx(pool: &PgPool, race: &GpxRace) -> Result<Uuid, sqlx::Error> {
    let distance_km = rust_decimal::Decimal::from_f64(race.distance_km)
        .unwrap_or_else(|| rust_decimal::Decimal::new(race.distance_km as i64, 0));
    let start_time_utc: chrono::DateTime<chrono::Utc> = race.start_time.into();

    // Upsert the race
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO races (id, name, year, start_time, distance_km, course_gpx)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)
         ON CONFLICT (name, year) DO UPDATE SET
             start_time = EXCLUDED.start_time,
             distance_km = EXCLUDED.distance_km,
             course_gpx = EXCLUDED.course_gpx,
             updated_at = NOW()
         RETURNING id",
    )
    .bind(&race.name)
    .bind(race.year)
    .bind(start_time_utc)
    .bind(distance_km)
    .bind(&race.gpx_xml)
    .fetch_one(pool)
    .await?;

    let race_id = row.0;

    // Upsert each checkpoint
    for (i, cp) in race.checkpoints.iter().enumerate() {
        let cp_distance = rust_decimal::Decimal::from_f64(cp.distance_km)
            .unwrap_or_else(|| rust_decimal::Decimal::new(cp.distance_km as i64, 0));
        let cp_lat = rust_decimal::Decimal::from_f64(cp.latitude)
            .unwrap_or_else(|| rust_decimal::Decimal::new(cp.latitude as i64, 0));
        let cp_lon = rust_decimal::Decimal::from_f64(cp.longitude)
            .unwrap_or_else(|| rust_decimal::Decimal::new(cp.longitude as i64, 0));
        let cp_ele = rust_decimal::Decimal::from_f64(cp.elevation_m)
            .unwrap_or_else(|| rust_decimal::Decimal::new(cp.elevation_m as i64, 0));
        let sort_order = i as i32;

        sqlx::query(
            "INSERT INTO checkpoints (id, race_id, name, distance_km, latitude, longitude, elevation_m, sort_order)
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (race_id, sort_order) DO UPDATE SET
                 name = EXCLUDED.name,
                 distance_km = EXCLUDED.distance_km,
                 latitude = EXCLUDED.latitude,
                 longitude = EXCLUDED.longitude,
                 elevation_m = EXCLUDED.elevation_m,
                 updated_at = NOW()",
        )
        .bind(race_id)
        .bind(&cp.name)
        .bind(cp_distance)
        .bind(cp_lat)
        .bind(cp_lon)
        .bind(cp_ele)
        .bind(sort_order)
        .execute(pool)
        .await?;
    }

    Ok(race_id)
}
