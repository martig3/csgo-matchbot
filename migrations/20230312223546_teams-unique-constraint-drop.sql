-- Add migration script here
ALTER TABLE teams
DROP CONSTRAINT teams_capitan_key;
ALTER TABLE teams
DROP CONSTRAINT teams_name_key;