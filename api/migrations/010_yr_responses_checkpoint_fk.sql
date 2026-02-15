-- Issue #5: Add checkpoint_id FK to yr_responses.
-- Replace fragile coordinate-equality lookups with a direct FK.

-- Step 1: Add nullable checkpoint_id column with FK
ALTER TABLE yr_responses ADD COLUMN checkpoint_id UUID REFERENCES checkpoints(id);

-- Step 2: Populate from checkpoints where coordinates match (rounded to yr.no precision).
-- This covers existing cached responses that line up with known checkpoints.
UPDATE yr_responses yr
SET checkpoint_id = cp.id
FROM checkpoints cp
WHERE ROUND(cp.latitude, 4) = yr.latitude
  AND ROUND(cp.longitude, 4) = yr.longitude
  AND ROUND(cp.elevation_m, 0) = yr.elevation_m;

-- Step 3: Delete any yr_responses rows that didn't match a checkpoint
-- (orphaned cache entries for locations no longer in the system).
DELETE FROM yr_responses WHERE checkpoint_id IS NULL;

-- Step 4: Make checkpoint_id NOT NULL now that all rows have a value
ALTER TABLE yr_responses ALTER COLUMN checkpoint_id SET NOT NULL;

-- Step 5: Drop the old coordinate-based unique index
DROP INDEX IF EXISTS idx_yr_responses_location;

-- Step 6: Create new unique index on checkpoint_id (one cache per checkpoint)
CREATE UNIQUE INDEX idx_yr_responses_checkpoint ON yr_responses(checkpoint_id);
