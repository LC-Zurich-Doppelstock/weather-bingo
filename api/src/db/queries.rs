use chrono::{DateTime, Utc};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{Checkpoint, Forecast, Race, YrCachedResponse};

/// Forecast time tolerance window (hours). SQL queries use a ±N hour BETWEEN
/// range so the composite index (checkpoint_id, forecast_time, fetched_at DESC)
/// drives the scan. This constant keeps the value in sync across all queries.
pub const FORECAST_TIME_TOLERANCE_HOURS: i32 = 3;

/// Internal helper for the batch forecast query — includes an `idx` column
/// from `WITH ORDINALITY` to preserve input ordering. All forecast fields are
/// `Option` because of the LEFT JOIN.
#[derive(Debug, sqlx::FromRow)]
pub struct ForecastWithIdx {
    pub idx: i64,

    // All forecast columns are Option due to LEFT JOIN LATERAL
    pub id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
    pub forecast_time: Option<DateTime<Utc>>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub source: Option<String>,
    pub temperature_c: Option<Decimal>,
    pub temperature_percentile_10_c: Option<Decimal>,
    pub temperature_percentile_90_c: Option<Decimal>,
    pub wind_speed_ms: Option<Decimal>,
    pub wind_speed_percentile_10_ms: Option<Decimal>,
    pub wind_speed_percentile_90_ms: Option<Decimal>,
    pub wind_direction_deg: Option<Decimal>,
    pub wind_gust_ms: Option<Decimal>,
    pub precipitation_mm: Option<Decimal>,
    pub precipitation_min_mm: Option<Decimal>,
    pub precipitation_max_mm: Option<Decimal>,
    pub humidity_pct: Option<Decimal>,
    pub dew_point_c: Option<Decimal>,
    pub cloud_cover_pct: Option<Decimal>,
    pub uv_index: Option<Decimal>,
    pub symbol_code: Option<String>,
    pub feels_like_c: Option<Decimal>,
    pub precipitation_type: Option<String>,
    pub yr_model_run_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

impl ForecastWithIdx {
    /// Convert to a Forecast if the LEFT JOIN found a matching row.
    /// Returns None if all forecast fields are NULL (no match).
    pub fn into_forecast(self) -> Option<Forecast> {
        Some(Forecast {
            id: self.id?,
            checkpoint_id: self.checkpoint_id?,
            forecast_time: self.forecast_time?,
            fetched_at: self.fetched_at?,
            source: self.source?,
            temperature_c: self.temperature_c?,
            temperature_percentile_10_c: self.temperature_percentile_10_c,
            temperature_percentile_90_c: self.temperature_percentile_90_c,
            wind_speed_ms: self.wind_speed_ms?,
            wind_speed_percentile_10_ms: self.wind_speed_percentile_10_ms,
            wind_speed_percentile_90_ms: self.wind_speed_percentile_90_ms,
            wind_direction_deg: self.wind_direction_deg?,
            wind_gust_ms: self.wind_gust_ms,
            precipitation_mm: self.precipitation_mm?,
            precipitation_min_mm: self.precipitation_min_mm,
            precipitation_max_mm: self.precipitation_max_mm,
            humidity_pct: self.humidity_pct?,
            dew_point_c: self.dew_point_c?,
            cloud_cover_pct: self.cloud_cover_pct?,
            uv_index: self.uv_index,
            symbol_code: self.symbol_code?,
            feels_like_c: self.feels_like_c?,
            precipitation_type: self.precipitation_type?,
            yr_model_run_at: self.yr_model_run_at,
            created_at: self.created_at?,
        })
    }
}
use crate::services::gpx::GpxRace;

/// Convert an f64 to a `Decimal`, falling back to a truncated integer representation
/// if the float cannot be exactly represented (e.g. NaN or infinity).
fn f64_to_dec(v: f64) -> Decimal {
    Decimal::from_f64(v).unwrap_or_else(|| Decimal::new(v as i64, 0))
}

/// Parameters for inserting a new forecast record.
pub struct InsertForecastParams {
    pub checkpoint_id: Uuid,
    pub forecast_time: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
    pub source: String,
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
    pub feels_like_c: Decimal,
    pub precipitation_type: String,
    pub yr_model_run_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// yr_responses CRUD
// ---------------------------------------------------------------------------

/// Get a cached yr.no response for a location, only if it hasn't expired.
pub async fn get_yr_cached_response(
    pool: &PgPool,
    latitude: Decimal,
    longitude: Decimal,
    elevation_m: Decimal,
) -> Result<Option<YrCachedResponse>, sqlx::Error> {
    sqlx::query_as::<_, YrCachedResponse>(
        "SELECT id, latitude, longitude, elevation_m, fetched_at, expires_at,
                last_modified, raw_response, created_at
         FROM yr_responses
         WHERE latitude = $1 AND longitude = $2 AND elevation_m = $3
           AND expires_at > NOW()",
    )
    .bind(latitude)
    .bind(longitude)
    .bind(elevation_m)
    .fetch_optional(pool)
    .await
}

/// Lightweight check: is a non-expired yr.no cached response available?
/// Returns true/false without transferring the large raw_response blob.
pub async fn is_yr_cache_valid(
    pool: &PgPool,
    latitude: Decimal,
    longitude: Decimal,
    elevation_m: Decimal,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> = sqlx::query_as(
        "SELECT 1 as exists_flag
         FROM yr_responses
         WHERE latitude = $1 AND longitude = $2 AND elevation_m = $3
           AND expires_at > NOW()
         LIMIT 1",
    )
    .bind(latitude)
    .bind(longitude)
    .bind(elevation_m)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

/// Batch check: which of the given locations have valid (non-expired) yr.no cache?
/// Returns the set of (latitude, longitude, elevation_m) tuples that are valid.
/// Executes as a single query regardless of how many locations are checked.
pub async fn get_valid_yr_cache_locations(
    pool: &PgPool,
    locations: &[(Decimal, Decimal, Decimal)],
) -> Result<Vec<(Decimal, Decimal, Decimal)>, sqlx::Error> {
    if locations.is_empty() {
        return Ok(Vec::new());
    }

    let lats: Vec<Decimal> = locations.iter().map(|(l, _, _)| *l).collect();
    let lons: Vec<Decimal> = locations.iter().map(|(_, l, _)| *l).collect();
    let eles: Vec<Decimal> = locations.iter().map(|(_, _, e)| *e).collect();

    let rows: Vec<(Decimal, Decimal, Decimal)> = sqlx::query_as(
        "SELECT yr.latitude, yr.longitude, yr.elevation_m
         FROM yr_responses yr
         INNER JOIN UNNEST($1::numeric[], $2::numeric[], $3::numeric[])
           AS loc(lat, lon, ele)
           ON yr.latitude = loc.lat AND yr.longitude = loc.lon AND yr.elevation_m = loc.ele
         WHERE yr.expires_at > NOW()",
    )
    .bind(&lats)
    .bind(&lons)
    .bind(&eles)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Get a cached yr.no response for a location regardless of expiry (for If-Modified-Since).
pub async fn get_yr_cached_response_any(
    pool: &PgPool,
    latitude: Decimal,
    longitude: Decimal,
    elevation_m: Decimal,
) -> Result<Option<YrCachedResponse>, sqlx::Error> {
    sqlx::query_as::<_, YrCachedResponse>(
        "SELECT id, latitude, longitude, elevation_m, fetched_at, expires_at,
                last_modified, raw_response, created_at
         FROM yr_responses
         WHERE latitude = $1 AND longitude = $2 AND elevation_m = $3",
    )
    .bind(latitude)
    .bind(longitude)
    .bind(elevation_m)
    .fetch_optional(pool)
    .await
}

/// Upsert (insert or update) a yr.no cached response for a location.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_yr_cached_response(
    pool: &PgPool,
    latitude: Decimal,
    longitude: Decimal,
    elevation_m: Decimal,
    fetched_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    last_modified: Option<&str>,
    raw_response: &serde_json::Value,
) -> Result<YrCachedResponse, sqlx::Error> {
    sqlx::query_as::<_, YrCachedResponse>(
        "INSERT INTO yr_responses (id, latitude, longitude, elevation_m, fetched_at, expires_at, last_modified, raw_response)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (latitude, longitude, elevation_m) DO UPDATE SET
             fetched_at = EXCLUDED.fetched_at,
             expires_at = EXCLUDED.expires_at,
             last_modified = EXCLUDED.last_modified,
             raw_response = EXCLUDED.raw_response
         RETURNING id, latitude, longitude, elevation_m, fetched_at, expires_at, last_modified, raw_response, created_at",
    )
    .bind(latitude)
    .bind(longitude)
    .bind(elevation_m)
    .bind(fetched_at)
    .bind(expires_at)
    .bind(last_modified)
    .bind(raw_response)
    .fetch_one(pool)
    .await
}

// ---------------------------------------------------------------------------
// Race queries
// ---------------------------------------------------------------------------

/// Get a race summary (no GPX blob) — lightweight existence check + metadata.
pub async fn get_race_summary(pool: &PgPool, id: Uuid) -> Result<Option<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(
        "SELECT id, name, year, start_time, distance_km FROM races WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// List all races (summary only, no GPX).
pub async fn list_races(pool: &PgPool) -> Result<Vec<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(
        "SELECT id, name, year, start_time, distance_km FROM races ORDER BY year DESC, name",
    )
    .fetch_all(pool)
    .await
}

/// Get just the GPX XML for a race (for course coordinate extraction).
pub async fn get_race_course_gpx(pool: &PgPool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT course_gpx FROM races WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
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
///
/// Uses a BETWEEN range (±3 hours) so the composite index on
/// (checkpoint_id, forecast_time, fetched_at DESC) is used for the scan,
/// then sorts by closeness within that window.
pub async fn get_latest_forecast(
    pool: &PgPool,
    checkpoint_id: Uuid,
    forecast_time: DateTime<Utc>,
) -> Result<Option<Forecast>, sqlx::Error> {
    let query = format!(
        "SELECT id, checkpoint_id, forecast_time, fetched_at, source,
                temperature_c, temperature_percentile_10_c, temperature_percentile_90_c,
                wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms,
                wind_direction_deg, wind_gust_ms,
                precipitation_mm, precipitation_min_mm, precipitation_max_mm,
                humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code,
                feels_like_c, precipitation_type, yr_model_run_at, created_at
         FROM forecasts
         WHERE checkpoint_id = $1
           AND forecast_time BETWEEN $2 - INTERVAL '{h} hours' AND $2 + INTERVAL '{h} hours'
         ORDER BY ABS(EXTRACT(EPOCH FROM (forecast_time - $2))),
                  yr_model_run_at DESC NULLS LAST,
                  fetched_at DESC
         LIMIT 1",
        h = FORECAST_TIME_TOLERANCE_HOURS,
    );
    sqlx::query_as::<_, Forecast>(&query)
        .bind(checkpoint_id)
        .bind(forecast_time)
        .fetch_optional(pool)
        .await
}

/// Batch get the latest forecast for multiple (checkpoint_id, forecast_time) pairs.
///
/// Returns one Forecast per input pair (in the same order), or None if no
/// forecast exists for that pair. Executes as a single query using LATERAL JOIN.
pub async fn get_latest_forecasts_batch(
    pool: &PgPool,
    pairs: &[(Uuid, DateTime<Utc>)],
) -> Result<Vec<Option<Forecast>>, sqlx::Error> {
    if pairs.is_empty() {
        return Ok(Vec::new());
    }

    let cp_ids: Vec<Uuid> = pairs.iter().map(|(id, _)| *id).collect();
    let times: Vec<DateTime<Utc>> = pairs.iter().map(|(_, t)| *t).collect();

    let query = format!(
        "SELECT
            p.idx,
            f.id, f.checkpoint_id, f.forecast_time, f.fetched_at, f.source,
            f.temperature_c, f.temperature_percentile_10_c, f.temperature_percentile_90_c,
            f.wind_speed_ms, f.wind_speed_percentile_10_ms, f.wind_speed_percentile_90_ms,
            f.wind_direction_deg, f.wind_gust_ms,
            f.precipitation_mm, f.precipitation_min_mm, f.precipitation_max_mm,
            f.humidity_pct, f.dew_point_c, f.cloud_cover_pct, f.uv_index, f.symbol_code,
            f.feels_like_c, f.precipitation_type, f.yr_model_run_at, f.created_at
         FROM UNNEST($1::uuid[], $2::timestamptz[])
              WITH ORDINALITY AS p(cp_id, ft, idx)
         LEFT JOIN LATERAL (
             SELECT *
             FROM forecasts
             WHERE checkpoint_id = p.cp_id
               AND forecast_time BETWEEN p.ft - INTERVAL '{h} hours' AND p.ft + INTERVAL '{h} hours'
             ORDER BY ABS(EXTRACT(EPOCH FROM (forecast_time - p.ft))),
                      yr_model_run_at DESC NULLS LAST,
                      fetched_at DESC
             LIMIT 1
         ) f ON true",
        h = FORECAST_TIME_TOLERANCE_HOURS,
    );
    let rows: Vec<ForecastWithIdx> = sqlx::query_as::<_, ForecastWithIdx>(&query)
        .bind(&cp_ids)
        .bind(&times)
        .fetch_all(pool)
        .await?;

    // Build result vector preserving input order
    let mut results: Vec<Option<Forecast>> = vec![None; pairs.len()];
    for row in rows {
        let idx = (row.idx - 1) as usize; // ORDINALITY is 1-based
                                          // If the LEFT JOIN found no match, the forecast fields will be NULL
        if let Some(forecast) = row.into_forecast() {
            results[idx] = Some(forecast);
        }
    }

    Ok(results)
}

/// Get forecast history for a checkpoint at a specific forecast time.
/// Returns all fetched versions, ordered by fetched_at ascending.
pub async fn get_forecast_history(
    pool: &PgPool,
    checkpoint_id: Uuid,
    forecast_time: DateTime<Utc>,
) -> Result<Vec<Forecast>, sqlx::Error> {
    let query = format!(
        "SELECT id, checkpoint_id, forecast_time, fetched_at, source,
                temperature_c, temperature_percentile_10_c, temperature_percentile_90_c,
                wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms,
                wind_direction_deg, wind_gust_ms,
                precipitation_mm, precipitation_min_mm, precipitation_max_mm,
                humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code,
                feels_like_c, precipitation_type, yr_model_run_at, created_at
         FROM forecasts
         WHERE checkpoint_id = $1
           AND forecast_time = (
               SELECT forecast_time FROM forecasts
               WHERE checkpoint_id = $1
                 AND forecast_time BETWEEN $2 - INTERVAL '{h} hours' AND $2 + INTERVAL '{h} hours'
               ORDER BY ABS(EXTRACT(EPOCH FROM (forecast_time - $2)))
               LIMIT 1
           )
         ORDER BY fetched_at ASC",
        h = FORECAST_TIME_TOLERANCE_HOURS,
    );
    sqlx::query_as::<_, Forecast>(&query)
        .bind(checkpoint_id)
        .bind(forecast_time)
        .fetch_all(pool)
        .await
}

/// Check if a forecast already exists for this (checkpoint, forecast_time, model_run).
/// Used for deduplication: re-fetching the same yr.no model run should not create
/// duplicate rows. If `yr_model_run_at` is None, always returns false (no dedup
/// possible without model run info).
///
/// Note: With the new bulk-insert architecture, deduplication is handled by
/// the unique index + ON CONFLICT DO NOTHING. Retained for potential future use.
#[allow(dead_code)]
pub async fn forecast_exists_for_model_run(
    pool: &PgPool,
    checkpoint_id: Uuid,
    forecast_time: DateTime<Utc>,
    yr_model_run_at: Option<DateTime<Utc>>,
) -> Result<bool, sqlx::Error> {
    let Some(model_run) = yr_model_run_at else {
        return Ok(false);
    };
    let row: Option<(i32,)> = sqlx::query_as(
        "SELECT 1 as exists_flag
         FROM forecasts
         WHERE checkpoint_id = $1
           AND forecast_time = $2
           AND yr_model_run_at = $3
         LIMIT 1",
    )
    .bind(checkpoint_id)
    .bind(forecast_time)
    .bind(model_run)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

/// Insert a new forecast record (append-only). No longer stores raw_response
/// (that lives in yr_responses now).
///
/// Note: With the new bulk-insert architecture, `bulk_insert_forecasts` is
/// preferred. Retained for potential future use.
#[allow(dead_code)]
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
            feels_like_c, precipitation_type, yr_model_run_at, created_at
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
                  feels_like_c, precipitation_type, yr_model_run_at, created_at",
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
    .bind(params.yr_model_run_at)
    .fetch_one(pool)
    .await
}

/// Bulk insert forecast records for a checkpoint from parsed yr.no timeseries.
///
/// Uses `ON CONFLICT DO NOTHING` on the deduplication index
/// (checkpoint_id, forecast_time, yr_model_run_at) to skip rows that already
/// exist for the same model run. Returns the number of rows actually inserted.
pub async fn bulk_insert_forecasts(
    pool: &PgPool,
    params: &[InsertForecastParams],
) -> Result<u64, sqlx::Error> {
    if params.is_empty() {
        return Ok(0);
    }

    // Build a batch of individual INSERTs wrapped in a single transaction.
    // This is simpler and more maintainable than building a multi-row VALUES clause.
    let mut tx = pool.begin().await?;
    let mut inserted = 0u64;

    for p in params {
        let result = sqlx::query(
            "INSERT INTO forecasts (
                id, checkpoint_id, forecast_time, fetched_at, source,
                temperature_c, temperature_percentile_10_c, temperature_percentile_90_c,
                wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms,
                wind_direction_deg, wind_gust_ms,
                precipitation_mm, precipitation_min_mm, precipitation_max_mm,
                humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code,
                feels_like_c, precipitation_type, yr_model_run_at, created_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9, $10, $11, $12, $13,
                $14, $15, $16, $17, $18, $19, $20, $21,
                $22, $23, $24, NOW()
            )
            ON CONFLICT (checkpoint_id, forecast_time, yr_model_run_at)
                WHERE yr_model_run_at IS NOT NULL
            DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(p.checkpoint_id)
        .bind(p.forecast_time)
        .bind(p.fetched_at)
        .bind(&p.source)
        .bind(p.temperature_c)
        .bind(p.temperature_percentile_10_c)
        .bind(p.temperature_percentile_90_c)
        .bind(p.wind_speed_ms)
        .bind(p.wind_speed_percentile_10_ms)
        .bind(p.wind_speed_percentile_90_ms)
        .bind(p.wind_direction_deg)
        .bind(p.wind_gust_ms)
        .bind(p.precipitation_mm)
        .bind(p.precipitation_min_mm)
        .bind(p.precipitation_max_mm)
        .bind(p.humidity_pct)
        .bind(p.dew_point_c)
        .bind(p.cloud_cover_pct)
        .bind(p.uv_index)
        .bind(&p.symbol_code)
        .bind(p.feels_like_c)
        .bind(&p.precipitation_type)
        .bind(p.yr_model_run_at)
        .execute(&mut *tx)
        .await?;
        inserted += result.rows_affected();
    }

    tx.commit().await?;
    Ok(inserted)
}

/// Get a single checkpoint by ID.
pub async fn get_checkpoint(
    pool: &PgPool,
    checkpoint_id: Uuid,
) -> Result<Option<Checkpoint>, sqlx::Error> {
    sqlx::query_as::<_, Checkpoint>(
        "SELECT id, race_id, name, distance_km, latitude, longitude, elevation_m, sort_order
         FROM checkpoints WHERE id = $1",
    )
    .bind(checkpoint_id)
    .fetch_optional(pool)
    .await
}

/// Upsert a race and its checkpoints from parsed GPX data.
///
/// Uses INSERT ON CONFLICT (name, year) for the race, and
/// INSERT ON CONFLICT (race_id, sort_order) for each checkpoint.
/// Returns the race UUID (existing or newly created).
pub async fn upsert_race_from_gpx(pool: &PgPool, race: &GpxRace) -> Result<Uuid, sqlx::Error> {
    let distance_km = f64_to_dec(race.distance_km);
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
        let cp_distance = f64_to_dec(cp.distance_km);
        let cp_lat = f64_to_dec(cp.latitude);
        let cp_lon = f64_to_dec(cp.longitude);
        let cp_ele = f64_to_dec(cp.elevation_m);
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
