-- Your SQL goes here
CREATE TABLE users
(
    id         SERIAL PRIMARY KEY,
    discord_id BIGINT NOT NULL,
    steam_id   VARCHAR NOT NULL
);