-- Issue #1: Add ON DELETE CASCADE to yr_responses.checkpoint_id FK.
-- Without CASCADE, deleting checkpoints (e.g. during re-seed) fails with FK violation.

-- Drop the existing FK constraint and re-add with CASCADE.
ALTER TABLE yr_responses
  DROP CONSTRAINT yr_responses_checkpoint_id_fkey,
  ADD CONSTRAINT yr_responses_checkpoint_id_fkey
    FOREIGN KEY (checkpoint_id) REFERENCES checkpoints(id) ON DELETE CASCADE;
