use super::schema::users;

#[derive(Queryable)]
pub struct User {
    pub id: i32,
    pub discord_id: i64,
    pub steam_id: String,
}
#[derive(Insertable)]
#[table_name="users"]
pub struct NewUser<'a> {
    pub discord_id: &'a i64,
    pub steam_id: &'a str,
}