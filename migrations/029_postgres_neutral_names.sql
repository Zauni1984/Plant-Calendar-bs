-- Neutral identifiers: the unlimited plan is now "nursery" ("Gärtnerei"),
-- formerly "csc"; the tent location type is "tent", formerly "growzelt".
-- The code still accepts the old values, so this only tidies stored data
-- (and what users see in their data export).
UPDATE users SET plan = 'nursery' WHERE plan = 'csc';
UPDATE subscriptions SET plan = 'nursery' WHERE plan = 'csc';
UPDATE subscription_history SET plan = 'nursery' WHERE plan = 'csc';
UPDATE subscription_history SET previous_plan = 'nursery' WHERE previous_plan = 'csc';
UPDATE locations SET location_type = 'tent' WHERE location_type = 'growzelt';
