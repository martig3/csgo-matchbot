use super::super::Context;
use crate::commands::steamid::SteamUser;
use anyhow::{Error, Result};
use matchbot_core::team::*;
use poise::command;
use serenity::model::{application::component::ButtonStyle, id::RoleId, user::User};

#[command(
    slash_command,
    guild_only,
    subcommands("create", "show", "leave", "invite", "kick")
)]
pub(crate) async fn team(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(slash_command, guild_only, subcommands("all"))]
pub(crate) async fn teams(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Show all teams")
)]
pub(crate) async fn all(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;
    let teams = Team::get_all(pool).await?;
    if teams.is_empty() {
        context.say("No teams found.").await?;
        return Ok(());
    }
    let mut all_teams = String::new();
    for (i, team) in teams.iter().enumerate() {
        let members = team.members(pool).await?;
        all_teams.push_str(format!("{}. ", i + 1).as_str());
        all_teams.push_str(team.format_team_str(members).await.as_str());
        all_teams.push_str("\n");
    }
    context.say(all_teams).await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Create a new team")
)]
pub(crate) async fn create(
    context: Context<'_>,
    #[description = "Team name"] name: String,
) -> Result<()> {
    if name.len() > 30 {
        context
            .say("Team name must be under 30 characters long")
            .await?;
    }
    let pool = &context.data().pool;

    let author = context.author().id;
    let guild = context
        .guild_id()
        .ok_or_else::<Error, _>(|| unreachable!())?;
    let steam_id = SteamUser::get_by_discord_id(pool, author.0 as i64).await?;
    if steam_id.is_none() {
        context
            .say("SteamID missing, add your steamId using `/steamid`")
            .await?;
        return Ok(());
    }

    // User does not have a team
    if let Some(team) = Team::get_by_member(pool, author.0 as i64).await? {
        if team.captain == author.0 as i64 {
            context
                .say(format!(
                    "You are already the captain of the <@&{role}> team!",
                    role = team.role
                ))
                .await?;
        } else {
            context
                .say(format!(
                    "You are already a member of the <@&{role}> team!",
                    role = team.role
                ))
                .await?;
        }
        return Ok(());
    }

    let role = guild
        .create_role(context.serenity_context(), |role| {
            role.name(name.clone()).mentionable(true)
        })
        .await?
        .id;

    if let Err(err) = create_team(pool, role.0, &name, author.0).await {
        guild.delete_role(context.serenity_context(), role).await?;
        return Err(err);
    }

    let mut member = guild.member(context.serenity_context(), author).await?;
    member.add_role(context.serenity_context(), role).await?;

    context.say(format!("Team <@&{role}> created!")).await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Show your team's roster")
)]
pub(crate) async fn show(
    context: Context<'_>,
    #[description = "Team role"] name: Option<RoleId>,
) -> Result<()> {
    let pool = &context.data().pool;

    let team: Team = match name {
        Some(role) => match Team::get_by_role(pool, role.0 as i64).await? {
            Some(team) => team,
            None => {
                context
                    .say(format!("Role <@&{role}> is not associated with a team!"))
                    .await?;
                return Ok(());
            }
        },
        None => match Team::get_by_member(pool, context.author().id.0 as i64).await? {
            Some(team) => team,
            None => {
                context.say("You are not on a team!").await?;
                return Ok(());
            }
        },
    };

    let members = team.members(pool).await?;
    let team_str = team.format_team_str(members).await;

    context.say(team_str).await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Leave your team")
)]
pub(crate) async fn leave(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;

    let author = context.author().id;
    let guild = context
        .guild_id()
        .ok_or_else::<Error, _>(|| unreachable!())?;

    // User has team
    let team = match Team::get_by_member(pool, author.0 as i64).await? {
        None => {
            context.say("You are not on a team!").await?;
            return Ok(());
        }
        Some(team) => team,
    };
    let members = team.members(pool).await?;

    let member_vec: Vec<u64> = members.clone().into_iter().map(|n| n as u64).collect();
    // User is not team captain OR is only member
    if author.0 == team.captain as u64 && [author.0] != member_vec.as_slice() {
        context
            .say("A captain cannot leave a team while it has members!")
            .await?;
        return Ok(());
    }

    Team::remove_member(pool, team.id, author.0 as i64).await?;
    let mut member = guild.member(context.serenity_context(), author.0).await?;
    member
        .remove_role(context.serenity_context(), team.role as u64)
        .await?;
    let member_vec: Vec<u64> = members.into_iter().map(|n| n as u64).collect();
    if [author.0] == member_vec.as_slice() {
        Team::delete(pool, team.id).await?;
        guild
            .delete_role(context.serenity_context(), team.role as u64)
            .await?;
        context.say("Team disbanded.").await?;
    } else {
        context.say("You left the team.").await?;
    }

    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Invite a user to your team")
)]
pub(crate) async fn invite(context: Context<'_>, user: User) -> Result<()> {
    let pool = &context.data().pool;
    let steam_user = SteamUser::get_by_discord_id(pool, user.id.0 as i64).await?;
    if steam_user.is_none() {
        context
            .say(
                "This user needs to add their steamId using the `/steamid` command before they can join a team",
            )
            .await?;
        return Ok(());
    }
    let guild = context
        .guild_id()
        .ok_or_else::<Error, _>(|| unreachable!())?;
    let author = context.author();

    // Author has team
    let team = match Team::get_by_member(pool, author.id.0 as i64).await? {
        None => {
            context.say("You are not on a team!").await?;
            return Ok(());
        }
        Some(team) => team,
    };

    // Author is team captain
    if author.id.0 != team.captain as u64 {
        context.say("You are not the captain of this team!").await?;
        return Ok(());
    }

    // User does not have team
    if let Some(user_team) = Team::get_by_member(pool, user.id.0 as i64).await? {
        if team.id == user_team.id {
            context.say("This user is already on your team!").await?;
        } else {
            context.say("This user is already on a team!").await?;
        }
        return Ok(());
    }

    let mut message = user
        .dm(context.serenity_context(), |message| {
            message
                .content(format!(
                    "You have been invited to join the <@&{}> team by <@{}>!",
                    team.role, author.id
                ))
                .components(|components| {
                    components.create_action_row(|row| {
                        row.create_button(|button| {
                            button
                                .style(ButtonStyle::Primary)
                                .label("Accept")
                                .custom_id("accepted")
                        })
                        .create_button(|button| {
                            button
                                .style(ButtonStyle::Danger)
                                .label("Decline")
                                .custom_id("declined")
                        })
                    })
                })
        })
        .await?;
    let reply = context.say("Invitation sent.").await?;

    let interaction = message
        .await_component_interaction(context.serenity_context())
        .author_id(user.id)
        .await;
    let response = match &interaction {
        Some(interaction) => interaction.data.custom_id.as_str(),
        None => {
            reply
                .edit(context, |message| message.content("Invitation expired."))
                .await?;
            return Ok(());
        }
    };

    reply
        .edit(context, |reply| {
            reply.content(format!(
                "{name} {response} the invitation.",
                name = user.name
            ))
        })
        .await?;
    message
        .edit(context.serenity_context(), |message| {
            message
                .content(format!("You have {response} the invitation!"))
                .set_components(Default::default())
        })
        .await?;

    match response {
        "accepted" => {
            Team::add_member(pool, team.id, user.id.0 as i64).await?;
            let mut member = guild.member(context.serenity_context(), user.id).await?;
            member
                .add_role(context.serenity_context(), team.role as u64)
                .await?;
        }
        "declined" => {}
        _ => unreachable!(),
    }

    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Kick a player from your team")
)]
pub(crate) async fn kick(context: Context<'_>, user: User) -> Result<()> {
    let pool = &context.data().pool;

    let guild = context
        .guild_id()
        .ok_or_else::<Error, _>(|| unreachable!())?;
    let author = context.author();

    // User is not Author
    if user.id == author.id {
        context
            .say("You cannot kick yourself from the team!")
            .await?;
        return Ok(());
    }

    // Author has team
    let team = match Team::get_by_member(pool, author.id.0 as i64).await? {
        None => {
            context.say("You are not on a team!").await?;
            return Ok(());
        }
        Some(team) => team,
    };

    // Author is team captain
    if author.id.0 != team.captain as u64 {
        context.say("You are not the captain of this team!").await?;
        return Ok(());
    }

    // User is on team, and it is author's team
    if let Some(user_team) = Team::get_by_member(pool, user.id.0 as i64).await? {
        if user_team.id != team.id {
            context
                .say(format!("<@{}> is not on your team!", user.id))
                .await?;
            return Ok(());
        }
    } else {
        context
            .say(format!("<@{}> is not on a team!", user.id))
            .await?;
        return Ok(());
    }

    Team::remove_member(pool, team.id, user.id.0 as i64).await?;

    let mut member = guild.member(context.serenity_context(), user.id).await?;
    member
        .remove_role(context.serenity_context(), team.role as u64)
        .await?;

    context
        .say(format!("You kicked <@{}> from the team.", user.id))
        .await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Transfer the captain role to another player")
)]
pub(crate) async fn transfer(
    context: Context<'_>,
    #[description = "User to assign captain"] user: User,
) -> Result<()> {
    let pool = &context.data().pool;

    let guild = context
        .guild_id()
        .ok_or_else::<Error, _>(|| unreachable!())?;
    let author = context.author();

    // User is not Author
    if user.id == author.id {
        context
            .say("You cannot transfer the team to yourself!")
            .await?;
        return Ok(());
    }

    // Author has team
    let team = match Team::get_by_member(pool, author.id.0 as i64).await? {
        None => {
            context.say("You are not on a team!").await?;
            return Ok(());
        }
        Some(team) => team,
    };

    // Author is team captain
    if author.id.0 != team.captain as u64 {
        context.say("You are not the captain of this team!").await?;
        return Ok(());
    }

    // User is on team, and it is author's team
    if let Some(user_team) = Team::get_by_member(pool, user.id.0 as i64).await? {
        if user_team.id != team.id {
            context
                .say(format!("<@{}> is not on your team!", user.id))
                .await?;
            return Ok(());
        }
    } else {
        context
            .say(format!("<@{}> is not on a team!", user.id))
            .await?;
        return Ok(());
    }

    Team::update_captain(pool, team.id, user.id.0 as i64).await?;

    let mut member = guild.member(context.serenity_context(), user.id).await?;
    member
        .remove_role(context.serenity_context(), team.role as u64)
        .await?;

    context
        .say(format!(
            "You have transferred the captain position to <@{}>.",
            user.id
        ))
        .await?;
    Ok(())
}
