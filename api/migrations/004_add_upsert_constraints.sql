-- Add unique constraints needed for GPX-based upsert seeding.
-- Races are unique by (name, year); checkpoints are unique within a race by sort_order.
ALTER TABLE races ADD CONSTRAINT uq_races_name_year UNIQUE (name, year);
ALTER TABLE checkpoints ADD CONSTRAINT uq_checkpoints_race_sort UNIQUE (race_id, sort_order);
