use super::super::Context;
use crate::commands::matches::{MatchSeries, SeriesType};
use anyhow::Result;
use poise::command;

use serenity::model::guild::Role;
use std::str::FromStr;

#[command(
    slash_command,
    guild_only,
    ephemeral,
    default_member_permissions = "MODERATE_MEMBERS",
    subcommands("addmatch", "deletematch")
)]
pub(crate) async fn admin(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(slash_command, guild_only, ephemeral)]
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

#[command(slash_command, guild_only, ephemeral)]
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
