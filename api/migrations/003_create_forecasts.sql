-- Create forecasts table (append-only: never overwrite, always insert new rows)
CREATE TABLE forecasts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    checkpoint_id UUID NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
    forecast_time TIMESTAMPTZ NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL,
    source VARCHAR(100) NOT NULL,

    -- Weather parameters from yr.no
    temperature_c DECIMAL NOT NULL,
    temperature_percentile_10_c DECIMAL,
    temperature_percentile_90_c DECIMAL,
    wind_speed_ms DECIMAL NOT NULL,
    wind_speed_percentile_10_ms DECIMAL,
    wind_speed_percentile_90_ms DECIMAL,
    wind_direction_deg DECIMAL NOT NULL,
    wind_gust_ms DECIMAL,
    precipitation_mm DECIMAL NOT NULL,
    precipitation_min_mm DECIMAL,
    precipitation_max_mm DECIMAL,
    humidity_pct DECIMAL NOT NULL,
    dew_point_c DECIMAL NOT NULL,
    cloud_cover_pct DECIMAL NOT NULL,
    uv_index DECIMAL,
    symbol_code VARCHAR(100) NOT NULL,

    -- Calculated by API (not from yr.no)
    feels_like_c DECIMAL NOT NULL,
    precipitation_type VARCHAR(50) NOT NULL,

    -- Raw API response for future use
    raw_response JSONB,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Fast lookup of latest forecast per checkpoint/time
CREATE INDEX idx_forecasts_checkpoint_time_fetched
    ON forecasts(checkpoint_id, forecast_time, fetched_at DESC);

-- Historical forecast queries
CREATE INDEX idx_forecasts_checkpoint_fetched
    ON forecasts(checkpoint_id, fetched_at);
