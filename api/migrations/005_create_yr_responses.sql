-- Cache yr.no full timeseries responses, keyed by location.
-- Uses yr.no's Expires/Last-Modified headers for cache validity
-- instead of an arbitrary staleness threshold.
CREATE TABLE yr_responses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    latitude DECIMAL(8,4) NOT NULL,
    longitude DECIMAL(8,4) NOT NULL,
    elevation_m DECIMAL(6,0) NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    last_modified TEXT,
    raw_response JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- One cached response per unique location (upserted on each fetch).
CREATE UNIQUE INDEX idx_yr_responses_location
    ON yr_responses(latitude, longitude, elevation_m);
