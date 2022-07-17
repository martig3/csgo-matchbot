#[macro_use]
extern crate diesel;
use diesel::{PgConnection, RunQueryDsl};
use self::models::{User, NewUser};
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