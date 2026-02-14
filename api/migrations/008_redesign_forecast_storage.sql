-- Migration 008: Redesign forecast storage to use yr.no native time slots.
--
-- Previously, forecast_time stored the pacing-derived pass-through time
-- (e.g. "skier arrives at checkpoint X at 09:14:07"). This caused:
-- - Different target_duration_hours produced different forecast_times, fragmenting data
-- - History was meaningless across different pacing assumptions
-- - Deduplication by (checkpoint_id, forecast_time, yr_model_run_at) didn't work well
--
-- Now, forecast_time stores yr.no's native timeseries timestamps (whole hours
-- or 6-hour intervals). The pacing model is applied at query time to find the
-- nearest stored yr.no slot.

-- 1. Delete all existing forecast rows (old fragmented data is useless)
DELETE FROM forecasts;

-- 2. Add unique constraint for proper deduplication.
--    Same (checkpoint, yr.no time slot, model run) should never be stored twice.
--    We use a partial unique index (WHERE yr_model_run_at IS NOT NULL) because
--    old rows with NULL model runs can't be deduplicated.
CREATE UNIQUE INDEX idx_forecasts_dedup
    ON forecasts (checkpoint_id, forecast_time, yr_model_run_at)
    WHERE yr_model_run_at IS NOT NULL;
