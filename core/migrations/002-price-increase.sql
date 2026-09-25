-- S04's price-increase flag, stored with the rest of the service summary so
-- the list does not re-derive it from every charge (PLAN.md 2.1). Rows from
-- an M1 scan read 0 until the next scan rebuilds them.
ALTER TABLE service ADD COLUMN price_increase INTEGER NOT NULL DEFAULT 0;
