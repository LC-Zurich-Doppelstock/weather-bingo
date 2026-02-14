use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

/// Health check response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Service status ("ok" when healthy)
    pub status: String,
    /// API version
    pub version: String,
}

/// Health check endpoint.
///
/// Returns the API status and version. Use this to verify the service is running.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
    )
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_returns_ok() {
        let response = health_check().await;
        assert_eq!(response.0.status, "ok");
        assert_eq!(response.0.version, env!("CARGO_PKG_VERSION"));
    }
}
