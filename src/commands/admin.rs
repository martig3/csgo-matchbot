use super::super::Context;
use crate::commands::matches::{MatchSeries, SeriesType};
use anyhow::Result;
use futures::{Stream, StreamExt};
use poise::command;
use std::str::FromStr;

use crate::commands::team::Team;
use serenity::model::guild::Role;
use sqlx::sqlx_macros::FromRow;
use sqlx::PgExecutor;
use strum::IntoEnumIterator;

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

async fn series_types<'a>(_ctx: Context<'_>, partial: &'a str) -> impl Stream<Item = String> + 'a {
    let s_types: Vec<SeriesType> = SeriesType::iter().collect::<Vec<_>>();
    let type_strings: Vec<String> = s_types.into_iter().map(|t| t.to_string()).collect();
    futures::stream::iter(type_strings)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.to_string())
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
    subcommands("add_match", "delete_match")
)]
pub(crate) async fn matches(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    default_member_permissions = "MODERATE_MEMBERS",
    subcommands("add_server", "delete_server", "show_servers")
)]
pub(crate) async fn servers(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    rename = "add",
    description_localized("en-US", "Add server template")
)]
pub(crate) async fn add_server(
    context: Context<'_>,
    #[description = "Location name"] location: String,
    #[description = "Dathost server id"] server_id: String,
) -> Result<()> {
    let pool = &context.data().pool;
    ServerTemplates::add(pool, location, server_id).await?;
    context.say("Server added").await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    rename = "delete",
    description_localized("en-US", "Delete server template")
)]
pub(crate) async fn delete_server(
    context: Context<'_>,
    #[description = "Location name"] location: String,
) -> Result<()> {
    let pool = &context.data().pool;
    ServerTemplates::delete(pool, location).await?;
    context.say("Server deleted").await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    rename = "show",
    description_localized("en-US", "Show all server templates")
)]
pub(crate) async fn show_servers(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;
    let servers = ServerTemplates::get_all(pool).await?;
    let content: String = servers
        .into_iter()
        .map(|s| format!("id: `{}` server_id: `{}`\n", s.location, s.server_id))
        .collect();
    context.say(content).await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    rename = "add",
    description_localized("en-US", "Add match to schedule")
)]
pub(crate) async fn add_match(
    context: Context<'_>,
    #[description = "Team One (Higher Seed)"] team_one: Role,
    #[description = "Team One (Lower Seed)"] team_two: Role,
    #[autocomplete = "series_types"] series_type: String,
) -> Result<()> {
    let pool = &context.data().pool;
    let series_type_enum = SeriesType::from_str(&series_type).unwrap();
    let team_one = Team::get_by_role(pool, team_one.id.0 as i64).await?;
    let team_two = Team::get_by_role(pool, team_two.id.0 as i64).await?;
    let result = MatchSeries::create(
        pool,
        team_one.unwrap().id,
        team_two.unwrap().id,
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

#[command(
    slash_command,
    guild_only,
    ephemeral,
    rename = "delete",
    description_localized("en-US", "Delete match from schedule")
)]
pub(crate) async fn delete_match(
    context: Context<'_>,
    #[description = "Match Id"] match_id: i32,
) -> Result<()> {
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
