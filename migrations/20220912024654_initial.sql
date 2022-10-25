CREATE TABLE steam_ids
(
    discord INT8 UNIQUE NOT NULL,
    steam   INT8 UNIQUE NOT NULL
);

CREATE TABLE teams
(
    id      SERIAL PRIMARY KEY,
    role    INT8 UNIQUE NOT NULL,
    name    TEXT UNIQUE NOT NULL,
    capitan INT8 UNIQUE NOT NULL
);

CREATE TABLE team_members
(
    team   INTEGER NOT NULL REFERENCES teams (id),
    member INT8    NOT NULL,
    UNIQUE (team, member)
);
CREATE INDEX ON team_members (team);
CREATE INDEX ON team_members (member);

CREATE TABLE maps
(
    id       SERIAL PRIMARY KEY,
    name     TEXT    NOT NULL,
    disabled BOOLEAN NOT NULL DEFAULT FALSE
);

INSERT INTO maps (name)
VALUES ('de_inferno'),
       ('de_vertigo'),
       ('de_overpass'),
       ('de_nuke'),
       ('de_dust2'),
       ('de_mirage'),
       ('de_ancient');

CREATE TYPE series_type AS ENUM ('bo1', 'bo3', 'bo5');
CREATE TABLE match_series
(
    id           SERIAL PRIMARY KEY,
    team_one     int8        NOT NULL REFERENCES teams (role),
    team_two     int8        NOT NULL REFERENCES teams (role),
    series_type  series_type NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ
);
CREATE INDEX ON match_series (team_one);
CREATE INDEX ON match_series (team_two);


create table servers
(
    id           serial primary key,
    match_series integer not null references match_series (id),
    server_id    text    not null,
    hostname     text    not null,
    game_port    integer not null,
    gotv_port    integer not null
);
CREATE TYPE vote_type AS ENUM ('pick', 'veto');
CREATE TABLE vote_info
(
    id           SERIAL PRIMARY KEY,
    match_series INTEGER   NOT NULL REFERENCES match_series (id),
    map          INTEGER   NOT NULL REFERENCES maps (id),
    type         vote_type NOT NULL,
    team         INTEGER   NOT NULL REFERENCES teams (id)
);
CREATE INDEX ON vote_info (match_series);

CREATE TABLE match
(
    id            SERIAL PRIMARY KEY,
    match_series  INTEGER NOT NULL REFERENCES match_series (id),
    map           INTEGER NOT NULL REFERENCES maps (id),
    picked_by     int8    NOT NULL REFERENCES teams (id),
    start_ct_team int8    NOT NULL REFERENCES teams (id),
    start_t_team  int8    NOT NULL REFERENCES teams (id),
    completed_at  timestamptz
);
CREATE INDEX ON match (match_series);

CREATE TABLE match_scores
(
    id             SERIAL PRIMARY KEY,
    match_id       INTEGER NOT NULL REFERENCES match (id),
    team_one_score INTEGER DEFAULT 0,
    team_two_score INTEGER DEFAULT 0
);

CREATE TABLE notes
(
    id           SERIAL PRIMARY KEY,
    match_series INTEGER NOT NULL REFERENCES match_series (id),
    note         TEXT
);
CREATE INDEX ON notes (match_series);

CREATE TABLE server_templates
(
    location  text primary key,
    server_id text not null
)