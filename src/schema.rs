table! {
    maps (name) {
        name -> Varchar,
    }
}

table! {
    match_servers (region_label) {
        region_label -> Varchar,
        server_id -> Varchar,
    }
}

table! {
    match_setup_step (id) {
        id -> Int4,
        match_id -> Int4,
        step_type -> Varchar,
        team_role_id -> Int8,
        map -> Nullable<Varchar>,
    }
}

table! {
    matches (id) {
        id -> Int4,
        team_one_role_id -> Int8,
        team_one_name -> Varchar,
        team_two_role_id -> Int8,
        team_two_name -> Varchar,
        note -> Nullable<Varchar>,
        date_added -> Timestamp,
        match_state -> Varchar,
        scheduled_time_str -> Nullable<Varchar>,
        series_type -> Varchar,
    }
}

table! {
    series_map (id) {
        id -> Int4,
        match_id -> Int4,
        map -> Varchar,
        picked_by_role_id -> Int8,
        start_attack_team_role_id -> Nullable<Int8>,
        start_defense_team_role_id -> Nullable<Int8>,
    }
}

table! {
    users (id) {
        id -> Int4,
        discord_id -> Int8,
        steam_id -> Varchar,
    }
}

joinable!(match_setup_step -> matches (match_id));
joinable!(series_map -> matches (match_id));

allow_tables_to_appear_in_same_query!(
    maps,
    match_servers,
    match_setup_step,
    matches,
    series_map,
    users,
);
