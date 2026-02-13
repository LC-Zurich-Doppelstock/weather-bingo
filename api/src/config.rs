/// Application configuration, parsed from environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub yr_user_agent: String,
    pub port: u16,
    pub forecast_staleness_secs: u64,
    /// Directory containing GPX files for race seeding.
    pub data_dir: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            yr_user_agent: std::env::var("YR_USER_AGENT").unwrap_or_else(|_| {
                "WeatherBingo/0.1 github.com/LC-Zurich-Doppelstock/weather-bingo".to_string()
            }),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a valid u16"),
            forecast_staleness_secs: std::env::var("FORECAST_STALENESS_SECS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .expect("FORECAST_STALENESS_SECS must be a valid u64"),
            data_dir: std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        // Clear env vars that might interfere
        std::env::set_var("DATABASE_URL", "postgres://test:test@localhost/test");
        std::env::remove_var("YR_USER_AGENT");
        std::env::remove_var("PORT");
        std::env::remove_var("FORECAST_STALENESS_SECS");
        std::env::remove_var("DATA_DIR");

        let config = AppConfig::from_env();

        assert_eq!(config.port, 8080);
        assert_eq!(config.forecast_staleness_secs, 60);
        assert!(config.yr_user_agent.contains("WeatherBingo"));
        assert_eq!(config.data_dir, "./data");
    }
}
