-- Add yr.no model run timestamp to forecasts table.
-- This tracks when yr.no's weather model actually generated the forecast,
-- as opposed to fetched_at which records when our app retrieved it.
-- Two fetches minutes apart may return the same model run; storing this
-- lets us deduplicate and show genuinely new forecast versions in history.
ALTER TABLE forecasts ADD COLUMN yr_model_run_at TIMESTAMPTZ;
