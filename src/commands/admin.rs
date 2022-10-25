use super::super::Context;
use crate::commands::matches::{MatchSeries, SeriesType};
use anyhow::Result;
use poise::command;

use serenity::model::guild::Role;
use sqlx::sqlx_macros::FromRow;
use sqlx::PgExecutor;
use std::str::FromStr;

#[derive(Debug, FromRow, Clone)]
pub struct ServerTemplates {
    pub location: String,
    pub server_id: String,
}

impl ServerTemplates {
    async fn add(
        executor: impl PgExecutor<'_>,
        location: String,
        server_id: String,
    ) -> Result<bool> {
        let result = sqlx::query!(
            "insert into server_templates (location, server_id) values ($1, $2)",
            location,
            server_id,
        )
        .execute(executor)
        .await?;
        return Ok(result.rows_affected() == 1);
    }
    async fn delete(executor: impl PgExecutor<'_>, location: String) -> Result<bool> {
        let result = sqlx::query!("delete from server_templates where location = $1", location,)
            .execute(executor)
            .await?;
        return Ok(result.rows_affected() == 1);
    }
    pub(crate) async fn get_all(executor: impl PgExecutor<'_>) -> Result<Vec<ServerTemplates>> {
        Ok(sqlx::query_as!(
            ServerTemplates,
            "select * from server_templates order by location"
        )
        .fetch_all(executor)
        .await?)
    }
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    default_member_permissions = "MODERATE_MEMBERS",
    subcommands("matches", "servers")
)]
pub(crate) async fn admin(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    default_member_permissions = "MODERATE_MEMBERS",
    subcommands("addmatch", "deletematch")
)]
pub(crate) async fn matches(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    default_member_permissions = "MODERATE_MEMBERS",
    subcommands("addserver", "deleteserver", "showservers")
)]
pub(crate) async fn servers(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(slash_command, guild_only, ephemeral, rename = "add")]
pub(crate) async fn addserver(
    context: Context<'_>,
    location: String,
    server_id: String,
) -> Result<()> {
    let pool = &context.data().pool;
    ServerTemplates::add(pool, location, server_id).await?;
    context.say("Server added").await?;
    Ok(())
}

#[command(slash_command, guild_only, ephemeral, rename = "delete")]
pub(crate) async fn deleteserver(context: Context<'_>, location: String) -> Result<()> {
    let pool = &context.data().pool;
    ServerTemplates::delete(pool, location).await?;
    context.say("Server deleted").await?;
    Ok(())
}

#[command(slash_command, guild_only, ephemeral, rename = "show")]
pub(crate) async fn showservers(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;
    let servers = ServerTemplates::get_all(pool).await?;
    let content: String = servers
        .into_iter()
        .map(|s| format!("id: `{}` server_id: `{}`\n", s.location, s.server_id))
        .collect();
    context.say(content).await?;
    Ok(())
}

#[command(slash_command, guild_only, ephemeral, rename = "add")]
pub(crate) async fn addmatch(
    context: Context<'_>,
    team_one: Role,
    team_two: Role,
    series_type: String,
) -> Result<()> {
    let pool = &context.data().pool;
    let series_type_enum = SeriesType::from_str(&series_type).unwrap();
    let result = MatchSeries::create(
        pool,
        team_one.id.0 as i64,
        team_two.id.0 as i64,
        series_type_enum,
    );
    if let Err(err) = result.await {
        log::error!("{:#?}", err);
        context.say("Error creating match").await?;
        return Ok(());
    }
    context.say("Match successfully created").await?;
    return Ok(());
}

#[command(slash_command, guild_only, ephemeral, rename = "delete")]
pub(crate) async fn deletematch(context: Context<'_>, match_id: i32) -> Result<()> {
    let pool = &context.data().pool;
    let result = MatchSeries::delete(pool, match_id).await;
    if let Err(_) = result {
        context
            .say("Could not delete match, please provide a valid match id")
            .await?;
        return Ok(());
    }
    context.say("Match successfully deleted").await?;
    return Ok(());
}
