// Weather Bingo API v0.1
use axum::{routing::get, Router};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod config;
mod db;
mod errors;
mod helpers;
mod routes;
mod services;

use config::AppConfig;
use routes::forecasts::AppState;
use services::poller::{PollerState, SharedPollerState};
use services::yr::YrClient;

/// Maximum number of connections in the database pool.
const DB_POOL_MAX_CONNECTIONS: u32 = 5;
/// Minimum number of connections kept alive in the database pool.
const DB_POOL_MIN_CONNECTIONS: u32 = 2;

/// Weather Bingo API — OpenAPI specification.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Weather Bingo API",
        version = "0.1.0",
        description = "Race-day weather forecasting API for cross-country skiing. \
            Fetches and caches weather forecasts from yr.no for race checkpoints, \
            calculates pass-through times using elevation-adjusted pacing, and provides \
            historical forecast data to track how predictions evolve.",
        license(name = "MIT"),
    ),
    tags(
        (name = "Health", description = "Service health check"),
        (name = "Races", description = "Race and checkpoint management"),
        (name = "Forecasts", description = "Weather forecast retrieval and history"),
        (name = "Poller", description = "Background forecast poller status"),
    ),
    paths(
        routes::health::health_check,
        routes::races::list_races,
        routes::races::get_race_course,
        routes::races::get_checkpoints,
        routes::forecasts::get_checkpoint_forecast,
        routes::forecasts::get_checkpoint_forecast_history,
        routes::forecasts::get_race_forecast,
        routes::poller::get_poller_status,
    ),
    components(
        schemas(
            routes::health::HealthResponse,
            routes::races::RaceListItem,
            services::gpx::CoursePoint,
            routes::races::CheckpointResponse,
            routes::forecasts::Weather,
            routes::forecasts::ForecastResponse,
            routes::forecasts::ForecastHistoryEntry,
            routes::forecasts::ForecastHistoryResponse,
            routes::forecasts::RaceForecastCheckpoint,
            routes::forecasts::RaceForecastResponse,
            services::poller::PollerState,
            services::poller::CheckpointPollStatus,
            errors::ErrorResponse,
        )
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "weather_bingo_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();

    // Set up database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(DB_POOL_MAX_CONNECTIONS)
        .min_connections(DB_POOL_MIN_CONNECTIONS)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Database migrations completed");

    // Seed races from GPX files
    let data_dir = std::path::Path::new(&config.data_dir);
    match services::gpx::load_races_from_dir(data_dir) {
        Ok(races) => {
            for race in &races {
                match db::queries::upsert_race_from_gpx(&pool, race).await {
                    Ok(race_id) => {
                        tracing::info!(
                            "Seeded race '{}' ({}) with {} checkpoints → id={}",
                            race.name,
                            race.year,
                            race.checkpoints.len(),
                            race_id
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to seed race '{}' ({}): {}",
                            race.name,
                            race.year,
                            e
                        );
                    }
                }
            }
            if races.is_empty() {
                tracing::warn!("No GPX files found in {}", data_dir.display());
            }
        }
        Err(e) => {
            tracing::error!(
                "Failed to load GPX files from {}: {}",
                data_dir.display(),
                e
            );
        }
    }

    // Create yr.no client
    let yr_client = YrClient::new(&config.yr_user_agent);

    // Build shared application state
    let app_state = AppState {
        pool: pool.clone(),
        yr_client: yr_client.clone(),
    };

    // Create shared poller state and spawn background poller
    let poller_state: SharedPollerState = Arc::new(RwLock::new(PollerState::new()));
    tokio::spawn(services::poller::run_poller(
        pool.clone(),
        yr_client,
        poller_state.clone(),
    ));

    // CORS — read-only API, restrict methods to GET; expose X-Forecast-Stale
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET])
        .allow_headers(Any)
        .expose_headers(["X-Forecast-Stale"
            .parse::<axum::http::HeaderName>()
            .unwrap()]);

    // Build router
    // Race routes use PgPool state directly; forecast routes use AppState.
    let race_routes = Router::new()
        .route("/api/v1/races", get(routes::races::list_races))
        .route(
            "/api/v1/races/:id/course",
            get(routes::races::get_race_course),
        )
        .route(
            "/api/v1/races/:id/checkpoints",
            get(routes::races::get_checkpoints),
        )
        .with_state(pool.clone());

    let forecast_routes = Router::new()
        .route(
            "/api/v1/forecasts/checkpoint/:checkpoint_id",
            get(routes::forecasts::get_checkpoint_forecast),
        )
        .route(
            "/api/v1/forecasts/checkpoint/:checkpoint_id/history",
            get(routes::forecasts::get_checkpoint_forecast_history),
        )
        .route(
            "/api/v1/forecasts/race/:race_id",
            get(routes::forecasts::get_race_forecast),
        )
        .with_state(app_state.clone());

    // Health check uses PgPool to verify DB connectivity
    let health_routes = Router::new()
        .route("/api/v1/health", get(routes::health::health_check))
        .with_state(pool);

    // Poller status uses SharedPollerState
    let poller_routes = Router::new()
        .route(
            "/api/v1/poller/status",
            get(routes::poller::get_poller_status),
        )
        .with_state(poller_state);

    let app = Router::new()
        .merge(health_routes)
        .merge(race_routes)
        .merge(forecast_routes)
        .merge(poller_routes)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(cors);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("API server listening on {}", addr);
    tracing::info!(
        "Swagger UI available at http://localhost:{}/swagger-ui/",
        config.port
    );

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener");
    axum::serve(listener, app)
        .await
        .expect("Server terminated unexpectedly");
}
