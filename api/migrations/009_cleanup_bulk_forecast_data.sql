-- Migration 009: Clean up bulk-inserted forecast data
--
-- The previous architecture bulk-inserted ALL yr.no timeseries entries (~240 per
-- location), most of which are irrelevant to race day. The new architecture only
-- stores forecasts for requested pass-through times.
--
-- This migration deletes all existing forecast rows so we start fresh.
-- The dedup index from migration 008 is kept — it's still useful.

DELETE FROM forecasts;
