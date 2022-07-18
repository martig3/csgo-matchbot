-- Your SQL goes here
CREATE TABLE matches
(
    id                 SERIAL PRIMARY KEY,
    team_one_role_id   bigint       NOT NULL,
    team_one_name      varchar(100) not null,
    team_two_role_id   bigint       NOT NULL,
    team_two_name      varchar(100) not null,
    note               varchar(500),
    date_added         timestamp    not null,
    match_state        varchar(50)  not null,
    scheduled_time_str varchar(100),
    series_type        varchar      NOT NULL
);

create table match_setup_step
(
    id           serial primary key,
    match_id     int4        not null references matches,
    step_type    varchar(50) NOT NULL,
    team_role_id bigint      not null,
    map          varchar(100)
);

create table series_map
(
    id                         serial primary key,
    match_id                   int4         not null references matches,
    map                        varchar(100) not null,
    picked_by_role_id          bigint       not null,
    start_attack_team_role_id  bigint,
    start_defense_team_role_id bigint
)