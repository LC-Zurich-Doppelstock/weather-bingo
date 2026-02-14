-- Raw yr.no response is now stored in yr_responses table.
-- Remove the (write-only) column from forecasts.
ALTER TABLE forecasts DROP COLUMN IF EXISTS raw_response;
