CREATE TABLE steam_ids (
    discord UINT8 UNIQUE NOT NULL,
    steam UINT8 UNIQUE
);

CREATE TABLE teams (
    id SERIAL PRIMARY KEY,
    role UINT8 UNIQUE NOT NULL,
    name TEXT UNIQUE NOT NULL,
    capitan UINT8 UNIQUE NOT NULL
);

CREATE TABLE team_members (
    team INTEGER NOT NULL REFERENCES teams (id),
    member UINT8 NOT NULL,
    UNIQUE (team, member)
);
CREATE INDEX ON team_members (team);
CREATE INDEX ON team_members (member);

CREATE TABLE maps (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    disabled BOOLEAN NOT NULL DEFAULT FALSE
);

INSERT INTO
    maps (name)
VALUES
    ('de_inferno'),
    ('de_vertigo'),
    ('de_overpass'),
    ('de_train'),
    ('de_lake'),
    ('de_cbble'),
    ('de_shortnuke');

CREATE TYPE series_type AS ENUM ('bo1', 'bo3', 'bo5');
CREATE TABLE match_series (
    id SERIAL PRIMARY KEY,
    team_one INTEGER NOT NULL REFERENCES teams (id),
    team_two INTEGER NOT NULL REFERENCES teams (id),
    series_type series_type NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    scheduled_at TIMESTAMPTZ
);
CREATE INDEX ON match_series (team_one);
CREATE INDEX ON match_series (team_two);

CREATE TYPE vote_type AS ENUM ('pick', 'veto');
CREATE TABLE vote_info (
    id SERIAL PRIMARY KEY,
    match_series INTEGER NOT NULL REFERENCES match_series (id),
    map INTEGER NOT NULL REFERENCES maps (id),
    type vote_type NOT NULL,
    team INTEGER NOT NULL REFERENCES teams (id)
);
CREATE INDEX ON vote_info (match_series);

CREATE TABLE match (
    id SERIAL PRIMARY KEY,
    match_series INTEGER NOT NULL REFERENCES match_series (id),
    map INTEGER NOT NULL REFERENCES maps (id),
    picked_by INTEGER NOT NULL REFERENCES teams (id),
    start_ct_team INTEGER NOT NULL REFERENCES teams (id),
    start_t_team INTEGER NOT NULL REFERENCES teams (id)
);
CREATE INDEX ON match (match_series);

CREATE TABLE notes (
    id SERIAL PRIMARY KEY,
    match_series INTEGER NOT NULL REFERENCES match_series (id),
    note TEXT
);
CREATE INDEX ON notes (match_series);