-- Add migration script here
CREATE TABLE tournament 
(
    id            SERIAL PRIMARY KEY,
    name text not null,
    started_at TIMESTAMPTZ not null,
    completed_at TIMESTAMPTZ 
);

INSERT INTO tournament (id, name, started_at, completed_at)
VALUES (0, 'default', now(), now());

alter table match_series
    add tournament integer default 0 not null REFERENCES tournament (id);

alter table teams 
    add tournament integer default 0 not null REFERENCES tournament (id);
alter table teams 
    add is_active boolean default true;