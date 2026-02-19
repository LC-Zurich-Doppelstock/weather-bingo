-- Add estimated snow surface temperature (API-computed field).
-- Nullable so existing rows are unaffected; new inserts will populate it.
ALTER TABLE forecasts ADD COLUMN snow_temperature_c DECIMAL;
