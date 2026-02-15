-- Issue #5: Close deduplication hole for forecasts with yr_model_run_at = NULL.
-- The existing partial unique index only covers rows WHERE yr_model_run_at IS NOT NULL.
-- This adds a second partial unique index for the NULL case, ensuring at most one
-- forecast row per (checkpoint_id, forecast_time) when yr_model_run_at is unknown.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forecasts_dedup_null_model_run
    ON forecasts (checkpoint_id, forecast_time)
    WHERE yr_model_run_at IS NULL;
