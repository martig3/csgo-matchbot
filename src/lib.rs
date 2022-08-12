#[macro_use]
extern crate diesel;

use diesel::{BoolExpressionMethods, EqAll, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl};
use diesel::associations::HasTable;
use crate::models::{Map, Match, MatchServer, MatchSetupStep, MatchState, NewMatch, NewMatchSetupStep, NewSeriesMap};
use crate::schema::matches::dsl::matches;
use self::models::{User, NewUser};
use crate::diesel::ExpressionMethods;
use crate::MatchState::Completed;
use crate::schema::maps::dsl::maps;
use crate::schema::match_servers::dsl::match_servers;
use crate::schema::matches::{match_state, scheduled_time_str};
use crate::schema::users::dsl::users;

pub mod schema;
pub mod models;

pub fn create_user<'a>(conn: &PgConnection, discord_id: &i64, steam_id: &str) -> User {
    use schema::users;

    let new_user = NewUser {
        discord_id,
        steam_id,
    };

    diesel::insert_into(users::table)
        .values(&new_user)
        .get_result(conn)
        .expect("Error saving new user")
}


pub fn get_user_by_discord_id<'a>(conn: &PgConnection, id: &i64) -> User {
    use crate::schema::users::discord_id;
    users
        .filter(discord_id.eq(id))
        .first::<User>(conn)
        .expect("Expected user")
}

pub fn create_match<'a>(conn: &PgConnection, new_match: NewMatch) -> usize {
    use schema::matches;

    diesel::insert_into(matches::table)
        .values(&new_match)
        .execute(conn)
        .expect("Error saving new user")
}

pub fn get_match<'a>(conn: &PgConnection, m_id: i32) -> Match {
    matches
        .find(m_id)
        .first::<Match>(conn)
        .expect("Expected match result")
}

pub fn get_matches<'a>(conn: &PgConnection, limit: i64, show_completed: bool) -> Vec<Match> {
    use crate::schema::matches::*;
    let mut query = matches::table().into_boxed();
    if show_completed {
        query = query.filter(match_state.eq(Completed));
    }
    query
        .order_by(id)
        .limit(limit)
        .load::<Match>(conn)
        .expect("Expected match result")
}

pub fn get_next_team_match<'a>(conn: &PgConnection, team_role_id: i64) -> Option<Match> {
    use crate::schema::matches::*;
    matches
        .filter(team_one_role_id.eq(team_role_id).or(team_two_role_id.eq(team_role_id)))
        .then_order_by(id)
        .first::<Match>(conn)
        .optional()
        .unwrap()
}

pub fn update_match_schedule<'a>(conn: &PgConnection, m_id: i32, time_str: String) -> Match {
    let updated_match = diesel::update(matches.find(m_id))
        .set(scheduled_time_str.eq(time_str))
        .get_result::<Match>(conn)
        .expect(&format!("unable to find match id: {}", m_id));
    updated_match
}

pub fn update_match_state<'a>(conn: &PgConnection, m_id: i32, state: MatchState) -> Match {
    let updated_match = diesel::update(matches.find(m_id))
        .set(match_state.eq(state))
        .get_result::<Match>(conn)
        .expect(&format!("unable to find match id: {}", m_id));
    updated_match
}

pub fn delete_match(conn: &PgConnection, m_id: i32) -> usize {
    use crate::schema::matches::*;
    diesel::delete(matches.filter(id.eq_all(m_id)))
        .execute(conn)
        .expect("Error deleting match")
}

pub fn create_match_setup_steps<'a>(conn: &PgConnection, new_steps: Vec<NewMatchSetupStep>) -> usize {
    use schema::match_setup_step;

    diesel::insert_into(match_setup_step::table)
        .values(&new_steps)
        .execute(conn)
        .expect("Error saving new setup step")
}

pub fn get_match_setup_steps<'a>(conn: &PgConnection, m_id: i32) -> Vec<MatchSetupStep> {
    use crate::schema::match_setup_step::dsl::*;
    match_setup_step
        .filter(match_id.eq_all(m_id))
        .load::<MatchSetupStep>(conn)
        .expect("Expected MatchSetupStep result")
}

pub fn create_series_maps<'a>(conn: &PgConnection, new_series_maps: Vec<NewSeriesMap>) -> usize {
    use schema::series_map;

    diesel::insert_into(series_map::table)
        .values(&new_series_maps)
        .execute(conn)
        .expect("Error saving new setup step")
}

pub fn get_map_pool<'a>(conn: &PgConnection) -> Vec<Map> {
    maps
        .load::<Map>(conn)
        .expect("Expected match result")
}

pub fn get_match_servers<'a>(conn: &PgConnection) -> Vec<MatchServer> {
    match_servers
        .load::<MatchServer>(conn)
        .expect("Expected match server result")
}