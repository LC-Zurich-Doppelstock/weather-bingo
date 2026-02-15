use axum::extract::State;
use axum::Json;
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

/// Health check response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Service status ("ok" when healthy, "degraded" when DB is unreachable)
    pub status: String,
    /// API version
    pub version: String,
    /// Whether the database is reachable
    pub database: bool,
}

/// Health check endpoint.
///
/// Returns the API status and version. Verifies database connectivity
/// with a simple query. Returns status "degraded" (still 200) if the
/// DB is unreachable, so load balancers can distinguish partial failures.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
    )
)]
pub async fn health_check(State(pool): State<PgPool>) -> Json<HealthResponse> {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&pool)
        .await
        .is_ok();

    Json(HealthResponse {
        status: if db_ok {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        database: db_ok,
    })
}

#[cfg(test)]
mod tests {
    // Health check now requires a PgPool via State extractor, so it cannot
    // be unit-tested without a real database. The endpoint is tested via
    // integration/manual testing with `docker compose up`.
    //
    // The old unit test called `health_check()` directly, which was possible
    // when it had no dependencies. With the DB check added, a mock pool
    // would be needed — but per project rules we use unit tests with mock
    // data only, not mock DB pools.
}
