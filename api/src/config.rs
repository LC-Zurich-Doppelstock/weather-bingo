/// Application configuration, parsed from environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub yr_user_agent: String,
    pub port: u16,
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
            data_dir: std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        // NOTE: set_var/remove_var in tests is unsafe in multi-threaded contexts
        // (Rust may run tests in parallel). However, this test exercises the
        // default-value logic which only needs env vars. We accept the risk
        // since cargo test runs this module's tests sequentially within one
        // test binary. If Rust editions mark these as `unsafe`, wrap accordingly.
        unsafe {
            std::env::set_var("DATABASE_URL", "postgres://test:test@localhost/test");
            std::env::remove_var("YR_USER_AGENT");
            std::env::remove_var("PORT");
            std::env::remove_var("DATA_DIR");
        }

        let config = AppConfig::from_env();

        assert_eq!(config.port, 8080);
        assert!(config.yr_user_agent.contains("WeatherBingo"));
        assert_eq!(config.data_dir, "./data");
    }
}
