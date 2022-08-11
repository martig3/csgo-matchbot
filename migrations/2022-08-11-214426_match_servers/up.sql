-- Your SQL goes here
create table match_servers
(
    region_label varchar(100) not null primary key,
    server_id    varchar      not null
);