use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{Checkpoint, Forecast, Race, YrCachedResponse};
use crate::helpers::f64_to_decimal_full;
use crate::services::gpx::GpxRace;

/// Forecast time tolerance window (hours). SQL queries use a ±N hour BETWEEN
/// range so the composite index (checkpoint_id, forecast_time, fetched_at DESC)
/// drives the scan. This compile-time constant is interpolated into SQL via
/// `format!` — this is safe because it's not user input. PostgreSQL doesn't
/// support `$N` bind parameters inside `INTERVAL` literals.
pub(crate) const FORECAST_TIME_TOLERANCE_HOURS: i32 = 3;

/// Forecast SELECT column list (no table alias).
///
/// All columns from the `forecasts` table, used in SELECT and RETURNING clauses.
/// Keep in sync with the `Forecast` model struct in `models.rs`.
const FORECAST_COLS: &str = "\
    id, checkpoint_id, forecast_time, fetched_at, source, \
    temperature_c, temperature_percentile_10_c, temperature_percentile_90_c, \
    wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms, \
    wind_direction_deg, wind_gust_ms, \
    precipitation_mm, precipitation_min_mm, precipitation_max_mm, \
    humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code, \
    feels_like_c, precipitation_type, snow_temperature_c, yr_model_run_at, created_at";

/// Forecast SELECT column list with `f.` table alias prefix.
///
/// Same columns as `FORECAST_COLS` but with `f.` prefix for use in JOINs.
const FORECAST_COLS_F: &str = "\
    f.id, f.checkpoint_id, f.forecast_time, f.fetched_at, f.source, \
    f.temperature_c, f.temperature_percentile_10_c, f.temperature_percentile_90_c, \
    f.wind_speed_ms, f.wind_speed_percentile_10_ms, f.wind_speed_percentile_90_ms, \
    f.wind_direction_deg, f.wind_gust_ms, \
    f.precipitation_mm, f.precipitation_min_mm, f.precipitation_max_mm, \
    f.humidity_pct, f.dew_point_c, f.cloud_cover_pct, f.uv_index, f.symbol_code, \
    f.feels_like_c, f.precipitation_type, f.snow_temperature_c, f.yr_model_run_at, f.created_at";

/// Forecast INSERT column list (excludes `id` and `created_at` which are auto-generated).
const FORECAST_INSERT_COLS: &str = "\
    id, checkpoint_id, forecast_time, fetched_at, source, \
    temperature_c, temperature_percentile_10_c, temperature_percentile_90_c, \
    wind_speed_ms, wind_speed_percentile_10_ms, wind_speed_percentile_90_ms, \
    wind_direction_deg, wind_gust_ms, \
    precipitation_mm, precipitation_min_mm, precipitation_max_mm, \
    humidity_pct, dew_point_c, cloud_cover_pct, uv_index, symbol_code, \
    feels_like_c, precipitation_type, snow_temperature_c, yr_model_run_at";

/// Internal helper for the batch forecast query — includes an `idx` column
/// from `WITH ORDINALITY` to preserve input ordering. All forecast fields are
/// `Option` because of the LEFT JOIN.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ForecastWithIdx {
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
    pub snow_temperature_c: Option<Decimal>,
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
            snow_temperature_c: self.snow_temperature_c,
            yr_model_run_at: self.yr_model_run_at,
            created_at: self.created_at?,
        })
    }
}

/// Parameters for inserting a new forecast record.
pub(crate) struct InsertForecastParams {
    pub(crate) checkpoint_id: Uuid,
    pub(crate) forecast_time: DateTime<Utc>,
    pub(crate) fetched_at: DateTime<Utc>,
    pub(crate) source: String,
    pub(crate) temperature_c: Decimal,
    pub(crate) temperature_percentile_10_c: Option<Decimal>,
    pub(crate) temperature_percentile_90_c: Option<Decimal>,
    pub(crate) wind_speed_ms: Decimal,
    pub(crate) wind_speed_percentile_10_ms: Option<Decimal>,
    pub(crate) wind_speed_percentile_90_ms: Option<Decimal>,
    pub(crate) wind_direction_deg: Decimal,
    pub(crate) wind_gust_ms: Option<Decimal>,
    pub(crate) precipitation_mm: Decimal,
    pub(crate) precipitation_min_mm: Option<Decimal>,
    pub(crate) precipitation_max_mm: Option<Decimal>,
    pub(crate) humidity_pct: Decimal,
    pub(crate) dew_point_c: Decimal,
    pub(crate) cloud_cover_pct: Decimal,
    pub(crate) uv_index: Option<Decimal>,
    pub(crate) symbol_code: String,
    pub(crate) feels_like_c: Decimal,
    pub(crate) precipitation_type: String,
    pub(crate) snow_temperature_c: Decimal,
    pub(crate) yr_model_run_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// yr_responses CRUD
// ---------------------------------------------------------------------------

/// Get a cached yr.no response for a checkpoint, only if it hasn't expired.
pub(crate) async fn get_yr_cached_response(
    pool: &PgPool,
    checkpoint_id: Uuid,
) -> Result<Option<YrCachedResponse>, sqlx::Error> {
    sqlx::query_as::<_, YrCachedResponse>(
        "SELECT id, checkpoint_id, latitude, longitude, elevation_m, fetched_at, expires_at,
                last_modified, raw_response, created_at
         FROM yr_responses
         WHERE checkpoint_id = $1
           AND expires_at > NOW()",
    )
    .bind(checkpoint_id)
    .fetch_optional(pool)
    .await
}

/// Get a cached yr.no response for a checkpoint regardless of expiry (for If-Modified-Since).
pub(crate) async fn get_yr_cached_response_any(
    pool: &PgPool,
    checkpoint_id: Uuid,
) -> Result<Option<YrCachedResponse>, sqlx::Error> {
    sqlx::query_as::<_, YrCachedResponse>(
        "SELECT id, checkpoint_id, latitude, longitude, elevation_m, fetched_at, expires_at,
                last_modified, raw_response, created_at
         FROM yr_responses
         WHERE checkpoint_id = $1",
    )
    .bind(checkpoint_id)
    .fetch_optional(pool)
    .await
}

/// Update expires_at and optionally last_modified on a yr.no cached response.
/// Used when yr.no returns 304 Not Modified with updated caching headers.
/// If `last_modified` is None, the existing value is preserved via COALESCE.
pub(crate) async fn update_yr_cache_expiry_and_last_modified(
    pool: &PgPool,
    checkpoint_id: Uuid,
    expires_at: DateTime<Utc>,
    last_modified: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE yr_responses SET expires_at = $2, last_modified = COALESCE($3, last_modified)
         WHERE checkpoint_id = $1",
    )
    .bind(checkpoint_id)
    .bind(expires_at)
    .bind(last_modified)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert (insert or update) a yr.no cached response for a checkpoint.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn upsert_yr_cached_response(
    pool: &PgPool,
    checkpoint_id: Uuid,
    latitude: Decimal,
    longitude: Decimal,
    elevation_m: Decimal,
    fetched_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    last_modified: Option<&str>,
    raw_response: &serde_json::Value,
) -> Result<YrCachedResponse, sqlx::Error> {
    sqlx::query_as::<_, YrCachedResponse>(
        "INSERT INTO yr_responses (id, checkpoint_id, latitude, longitude, elevation_m, fetched_at, expires_at, last_modified, raw_response)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (checkpoint_id) DO UPDATE SET
             latitude = EXCLUDED.latitude,
             longitude = EXCLUDED.longitude,
             elevation_m = EXCLUDED.elevation_m,
             fetched_at = EXCLUDED.fetched_at,
             expires_at = EXCLUDED.expires_at,
             last_modified = EXCLUDED.last_modified,
             raw_response = EXCLUDED.raw_response
         RETURNING id, checkpoint_id, latitude, longitude, elevation_m, fetched_at, expires_at, last_modified, raw_response, created_at",
    )
    .bind(checkpoint_id)
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
pub(crate) async fn get_race_summary(pool: &PgPool, id: Uuid) -> Result<Option<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(
        "SELECT id, name, year, start_time, distance_km FROM races WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// List all races (summary only, no GPX).
pub(crate) async fn list_races(pool: &PgPool) -> Result<Vec<Race>, sqlx::Error> {
    sqlx::query_as::<_, Race>(
        "SELECT id, name, year, start_time, distance_km FROM races ORDER BY year DESC, name",
    )
    .fetch_all(pool)
    .await
}

/// Get just the GPX XML for a race (for course coordinate extraction).
pub(crate) async fn get_race_course_gpx(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT course_gpx FROM races WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

/// Get all checkpoints for a race, ordered by sort_order.
pub(crate) async fn get_checkpoints(
    pool: &PgPool,
    race_id: Uuid,
) -> Result<Vec<Checkpoint>, sqlx::Error> {
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
pub(crate) async fn get_latest_forecast(
    pool: &PgPool,
    checkpoint_id: Uuid,
    forecast_time: DateTime<Utc>,
) -> Result<Option<Forecast>, sqlx::Error> {
    let query = format!(
        "SELECT {FORECAST_COLS}
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
pub(crate) async fn get_latest_forecasts_batch(
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
            {FORECAST_COLS_F}
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
        if idx >= results.len() {
            tracing::warn!(
                "get_latest_forecasts_batch: ORDINALITY index {} out of bounds (len={}), skipping",
                row.idx,
                pairs.len(),
            );
            continue;
        }
        // If the LEFT JOIN found no match, the forecast fields will be NULL
        if let Some(forecast) = row.into_forecast() {
            results[idx] = Some(forecast);
        }
    }

    Ok(results)
}

/// Maximum number of history entries returned per checkpoint.
/// Prevents unbounded result sets for long-running forecast tracking.
pub(crate) const MAX_FORECAST_HISTORY_ENTRIES: i32 = 200;

/// Get forecast history for a checkpoint at a specific forecast time.
///
/// Returns one entry per yr.no model run (deduplicated server-side),
/// ordered by model run time ascending. When `yr_model_run_at` is NULL
/// (pre-poller legacy rows), `fetched_at` is used as the fallback via
/// `COALESCE`. For each model run, only the latest `fetched_at` is kept.
pub(crate) async fn get_forecast_history(
    pool: &PgPool,
    checkpoint_id: Uuid,
    forecast_time: DateTime<Utc>,
) -> Result<Vec<Forecast>, sqlx::Error> {
    let query = format!(
        "SELECT DISTINCT ON (COALESCE(yr_model_run_at, fetched_at))
             {FORECAST_COLS}
         FROM forecasts
         WHERE checkpoint_id = $1
           AND forecast_time = (
               SELECT forecast_time FROM forecasts
               WHERE checkpoint_id = $1
                 AND forecast_time BETWEEN $2 - INTERVAL '{h} hours' AND $2 + INTERVAL '{h} hours'
               ORDER BY ABS(EXTRACT(EPOCH FROM (forecast_time - $2)))
               LIMIT 1
           )
         ORDER BY COALESCE(yr_model_run_at, fetched_at) ASC, fetched_at DESC
         LIMIT {limit}",
        h = FORECAST_TIME_TOLERANCE_HOURS,
        limit = MAX_FORECAST_HISTORY_ENTRIES,
    );
    sqlx::query_as::<_, Forecast>(&query)
        .bind(checkpoint_id)
        .bind(forecast_time)
        .fetch_all(pool)
        .await
}

/// Insert a single forecast record, deduplicating by
/// `(checkpoint_id, forecast_time, yr_model_run_at)`.
///
/// Uses `ON CONFLICT DO NOTHING` so re-inserting the same yr.no time slot
/// from the same model run is a no-op. Handles both cases:
/// - yr_model_run_at IS NOT NULL → partial unique index on 3 columns
/// - yr_model_run_at IS NULL → partial unique index on (checkpoint_id, forecast_time)
///
/// Returns `Some(Forecast)` when a new row was inserted, or `None` when
/// it already existed (deduplicated).
pub(crate) async fn insert_forecast(
    pool: &PgPool,
    p: InsertForecastParams,
) -> Result<Option<Forecast>, sqlx::Error> {
    // Choose the appropriate ON CONFLICT clause based on whether yr_model_run_at is set.
    // PostgreSQL requires the conflict target to match a specific unique index.
    let sql = if p.yr_model_run_at.is_some() {
        format!(
            "INSERT INTO forecasts ({FORECAST_INSERT_COLS})
             VALUES (
                gen_random_uuid(), $1, $2, $3, $4,
                $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24
             )
             ON CONFLICT (checkpoint_id, forecast_time, yr_model_run_at)
                WHERE yr_model_run_at IS NOT NULL
             DO NOTHING
             RETURNING {FORECAST_COLS}"
        )
    } else {
        format!(
            "INSERT INTO forecasts ({FORECAST_INSERT_COLS})
             VALUES (
                gen_random_uuid(), $1, $2, $3, $4,
                $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24
             )
             ON CONFLICT (checkpoint_id, forecast_time)
                WHERE yr_model_run_at IS NULL
             DO NOTHING
             RETURNING {FORECAST_COLS}"
        )
    };

    sqlx::query_as::<_, Forecast>(&sql)
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
        .bind(p.snow_temperature_c)
        .bind(p.yr_model_run_at)
        .fetch_optional(pool)
        .await
}

/// Get a single checkpoint by ID.
pub(crate) async fn get_checkpoint(
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
/// Deletes orphan checkpoints that no longer exist in the GPX.
/// All operations run within a single transaction.
/// Returns the race UUID (existing or newly created).
pub(crate) async fn upsert_race_from_gpx(
    pool: &PgPool,
    race: &GpxRace,
) -> Result<Uuid, sqlx::Error> {
    let distance_km = f64_to_decimal_full(race.distance_km);
    let start_time_utc: chrono::DateTime<chrono::Utc> = race.start_time.into();

    let mut tx = pool.begin().await?;

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
    .fetch_one(&mut *tx)
    .await?;

    let race_id = row.0;

    // Upsert each checkpoint
    for (i, cp) in race.checkpoints.iter().enumerate() {
        let cp_distance = f64_to_decimal_full(cp.distance_km);
        let cp_lat = f64_to_decimal_full(cp.latitude);
        let cp_lon = f64_to_decimal_full(cp.longitude);
        let cp_ele = f64_to_decimal_full(cp.elevation_m);
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
        .execute(&mut *tx)
        .await?;
    }

    // Delete orphan checkpoints whose sort_order is beyond the new checkpoint count.
    // This handles the case where a re-seed has fewer checkpoints than before.
    let max_sort_order = race.checkpoints.len() as i32;
    sqlx::query("DELETE FROM checkpoints WHERE race_id = $1 AND sort_order >= $2")
        .bind(race_id)
        .bind(max_sort_order)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(race_id)
}

// ---------------------------------------------------------------------------
// Poller queries
// ---------------------------------------------------------------------------

/// A race with all its checkpoints, used by the background poller.
pub(crate) struct RaceWithCheckpoints {
    pub(crate) race: Race,
    pub(crate) checkpoints: Vec<Checkpoint>,
}

/// Get all races with start_time within the next `lookahead_days` days,
/// along with their checkpoints. Used by the background poller to know
/// which checkpoints need proactive forecast fetching.
pub(crate) async fn get_upcoming_races_with_checkpoints(
    pool: &PgPool,
    lookahead_days: i64,
) -> Result<Vec<RaceWithCheckpoints>, sqlx::Error> {
    let races = sqlx::query_as::<_, Race>(
        "SELECT id, name, year, start_time, distance_km
         FROM races
         WHERE start_time BETWEEN NOW() - INTERVAL '1 day'
           AND NOW() + $1 * INTERVAL '1 day'
         ORDER BY start_time",
    )
    .bind(lookahead_days as f64)
    .fetch_all(pool)
    .await?;

    let mut results = Vec::with_capacity(races.len());
    for race in races {
        let checkpoints = get_checkpoints(pool, race.id).await?;
        results.push(RaceWithCheckpoints { race, checkpoints });
    }
    Ok(results)
}

/// Get the earliest expires_at timestamp across yr_responses for the given checkpoint IDs.
/// Returns None if no yr_responses rows exist for any of the checkpoint IDs.
pub(crate) async fn get_earliest_expiry(
    pool: &PgPool,
    checkpoint_ids: &[Uuid],
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    if checkpoint_ids.is_empty() {
        return Ok(None);
    }
    let row: Option<(Option<DateTime<Utc>>,)> =
        sqlx::query_as("SELECT MIN(expires_at) FROM yr_responses WHERE checkpoint_id = ANY($1)")
            .bind(checkpoint_ids)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|r| r.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    // f64_to_decimal_full tests are now in helpers.rs
    // Tests here should focus on query-specific logic.

    #[test]
    fn test_forecast_time_tolerance_hours_is_positive() {
        assert!(FORECAST_TIME_TOLERANCE_HOURS > 0);
    }
}
