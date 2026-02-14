use axum::extract::{Path, State};
use axum::Json;
use rust_decimal::prelude::ToPrimitive;
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::{models, queries};
use crate::errors::{AppError, ErrorResponse};
use crate::services::gpx::CoursePoint;

/// Response type for GET /api/v1/races (list, without GPX).
#[derive(Debug, Serialize, ToSchema)]
pub struct RaceListItem {
    /// Unique race identifier
    pub id: Uuid,
    /// Race name (e.g. "Vasaloppet")
    pub name: String,
    /// Race year
    pub year: i32,
    /// Race start time in ISO 8601 / RFC 3339 format
    pub start_time: String,
    /// Total race distance in kilometres
    pub distance_km: f64,
}

impl From<models::Race> for RaceListItem {
    fn from(r: models::Race) -> Self {
        Self {
            id: r.id,
            name: r.name,
            year: r.year,
            start_time: r.start_time.to_rfc3339(),
            distance_km: r.distance_km.to_f64().unwrap_or(0.0),
        }
    }
}

/// Response type for GET /api/v1/races/:id/checkpoints.
#[derive(Debug, Serialize, ToSchema)]
pub struct CheckpointResponse {
    /// Unique checkpoint identifier
    pub id: Uuid,
    /// Checkpoint name (e.g. "Smagan")
    pub name: String,
    /// Distance from race start in kilometres
    pub distance_km: f64,
    /// Latitude (WGS84)
    pub latitude: f64,
    /// Longitude (WGS84)
    pub longitude: f64,
    /// Elevation in metres above sea level
    pub elevation_m: f64,
    /// Display order along the course
    pub sort_order: i32,
}

impl From<models::Checkpoint> for CheckpointResponse {
    fn from(c: models::Checkpoint) -> Self {
        Self {
            id: c.id,
            name: c.name,
            distance_km: c.distance_km.to_f64().unwrap_or(0.0),
            latitude: c.latitude.to_f64().unwrap_or(0.0),
            longitude: c.longitude.to_f64().unwrap_or(0.0),
            elevation_m: c.elevation_m.to_f64().unwrap_or(0.0),
            sort_order: c.sort_order,
        }
    }
}

/// List all available races.
#[utoipa::path(
    get,
    path = "/api/v1/races",
    tag = "Races",
    responses(
        (status = 200, description = "List of all races", body = Vec<RaceListItem>),
    )
)]
pub async fn list_races(State(pool): State<PgPool>) -> Result<Json<Vec<RaceListItem>>, AppError> {
    let races = queries::list_races(&pool).await?;
    let items: Vec<RaceListItem> = races.into_iter().map(RaceListItem::from).collect();
    Ok(Json(items))
}

/// Get race course as pre-parsed JSON coordinates.
#[utoipa::path(
    get,
    path = "/api/v1/races/{id}/course",
    tag = "Races",
    params(
        ("id" = Uuid, Path, description = "Race UUID"),
    ),
    responses(
        (status = 200, description = "Course coordinates as [lat, lon, ele] points", body = Vec<CoursePoint>),
        (status = 404, description = "Race not found", body = ErrorResponse),
    )
)]
pub async fn get_race_course(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<CoursePoint>>, AppError> {
    let gpx = queries::get_race_course_gpx(&pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Race {} not found", id)))?;
    let points = crate::services::gpx::extract_track_points(&gpx)
        .map_err(|e| AppError::InternalError(format!("Failed to parse course GPX: {}", e)))?;
    Ok(Json(points))
}

/// Get all checkpoints for a race, ordered by distance from start.
#[utoipa::path(
    get,
    path = "/api/v1/races/{id}/checkpoints",
    tag = "Races",
    params(
        ("id" = Uuid, Path, description = "Race UUID"),
    ),
    responses(
        (status = 200, description = "List of checkpoints along the course", body = Vec<CheckpointResponse>),
        (status = 404, description = "Race not found", body = ErrorResponse),
    )
)]
pub async fn get_checkpoints(
    State(pool): State<PgPool>,
    Path(race_id): Path<Uuid>,
) -> Result<Json<Vec<CheckpointResponse>>, AppError> {
    // Verify the race exists first (lightweight — no GPX blob)
    let _race = queries::get_race_summary(&pool, race_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Race {} not found", race_id)))?;

    let checkpoints = queries::get_checkpoints(&pool, race_id).await?;
    let items: Vec<CheckpointResponse> = checkpoints
        .into_iter()
        .map(CheckpointResponse::from)
        .collect();
    Ok(Json(items))
}
