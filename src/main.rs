
use anyhow::Error;
use dotenvy::{dotenv, var};
use poise::{Event, Framework, FrameworkOptions, samples::create_application_commands};
use serenity::model::gateway::GatewayIntents;
use sqlx::{PgPool, migrate::Migrator};

static MIGRATOR: Migrator = sqlx::migrate!();

struct Data {
    pool: PgPool,
}

type Context<'a> = poise::Context<'a, Data, Error>;

mod commands {
    pub(crate) use admin::admin;
    pub(crate) use team::team;

    use super::Context;

    pub(crate) mod admin {
        use anyhow::Result;
        use poise::command;

        use super::Context;

        #[command(slash_command, guild_only, ephemeral, default_member_permissions = "MODERATE_MEMBERS")]
        pub(crate) async fn admin(context: Context<'_>) -> Result<()> {
            context.say("Nothing to see here...").await?;
            Ok(())
        }
    }

    pub(crate) mod team {
        use anyhow::{Error, Result};
        use poise::command;
        use serenity::model::{
            application::component::ButtonStyle,
            id::RoleId,
            user::User,
        };
        use sqlx::{PgExecutor, PgPool};

        use super::Context;

        #[allow(unused)]
        #[derive(Debug)]
        pub struct Team {
            id: i32,
            role: u64,
            name: String,
            capitan: u64,
        }

        #[allow(unused)]
        impl Team {
            pub async fn create(executor: impl PgExecutor<'_>, role: u64, name: &str, capitan: u64) -> Result<Team> {
                Ok(sqlx::query_as!(
                    Team,
                    "INSERT INTO teams
                        (role, name, capitan)
                    VALUES
                        ($1, $2, $3)
                    RETURNING *",
                    role,
                    name,
                    capitan
                ).fetch_one(executor).await?)
            }

            pub async fn delete(executor: impl PgExecutor<'_>, team: i32) -> Result<bool> {
                let result = sqlx::query!("DELETE FROM teams WHERE id = $1", team)
                    .execute(executor)
                    .await?;

                Ok(result.rows_affected() == 1)
            }

            pub async fn add_member(executor: impl PgExecutor<'_>, team: i32, member: u64) -> Result<bool> {
                let result = sqlx::query!(
                    "INSERT INTO team_members (team, member) VALUES ($1, $2)",
                    team,
                    member
                ).execute(executor).await?;

                Ok(result.rows_affected() == 1)
            }

            pub async fn remove_member(executor: impl PgExecutor<'_>, team: i32, member: u64) -> Result<bool> {
                let result = sqlx::query!(
                    "DELETE FROM team_members WHERE team = $1 AND member = $2",
                    team,
                    member
                ).execute(executor).await?;

                Ok(result.rows_affected() == 1)
            }

            pub async fn get_by_role(executor: impl PgExecutor<'_>, role: u64) -> Result<Option<Team>> {                
                Ok(sqlx::query_as!(
                    Team,
                    "SELECT * FROM teams WHERE role = $1",
                    role
                ).fetch_optional(executor).await?)
            }

            pub async fn get_by_member(executor: impl PgExecutor<'_>, member: u64) -> Result<Option<Team>> {
                Ok(sqlx::query_as!(
                    Team,
                    "SELECT teams.*
                     FROM team_members
                     JOIN teams
                        ON team_members.team = teams.id
                     WHERE team_members.member = $1",
                    member
                ).fetch_optional(executor).await?)
            }

            pub async fn members(&self, executor: impl PgExecutor<'_>) -> Result<Vec<u64>> {
                Ok(sqlx::query_scalar!(
                    "SELECT member FROM team_members WHERE team = $1",
                    self.id
                )
                    .fetch_all(executor)
                    .await?)
            }

            pub async fn update_capitan(executor: impl PgExecutor<'_>, team: i32, member: u64) -> Result<bool> {
                let result = sqlx::query!(
                    "UPDATE teams SET capitan = $1 WHERE id = $2",
                    member,
                    team
                )
                    .execute(executor)
                    .await?;
                Ok(result.rows_affected() == 1)
            }
        }

        #[command(slash_command, guild_only, subcommands("create", "show", "leave", "invite", "kick"))]
        pub(crate) async fn team(_context: Context<'_>) -> Result<()> { Ok(()) }


        async fn create_team(pool: &PgPool, role: u64, name: impl AsRef<str>, capitan: u64) -> Result<()> {
            let mut transaction = pool.begin().await?;
            let team = Team::create(&mut transaction, role, name.as_ref(), capitan).await?;
            Team::add_member(&mut transaction, team.id, capitan).await?;
            transaction.commit().await?;
            Ok(())
        }

        #[command(slash_command, guild_only, ephemeral)]
        pub(crate) async fn create(context: Context<'_>, name: String) -> Result<()> {
            let pool = &context.data().pool;
            
            let author = context.author().id;
            let guild = context.guild_id().ok_or_else::<Error, _>(|| unreachable!())?;

            // User does not have a team
            if let Some(team) = Team::get_by_member(pool, author.0).await? {
                if team.capitan == author.0 {
                    context.say(format!("You are already the capitan of the <@&{role}> team!", role = team.role)).await?;
                } else {
                    context.say(format!("You are already a member of the <@&{role}> team!", role = team.role)).await?;
                }
                return Ok(());
            }

            let role = guild.create_role(context.discord(), |role| {
                role
                    .name(name.clone())
                    .mentionable(true)
            }).await?.id;

            if let Err(err) = create_team(pool, role.0, &name, author.0).await {
                guild.delete_role(context.discord(), role).await?;
                return Err(err);
            }

            let mut member = guild.member(context.discord(), author).await?;
            member.add_role(context.discord(), role).await?;

            context.say(format!("Team <@&{role}> created!")).await?;
            Ok(())
        }

        #[command(slash_command, guild_only, ephemeral)]
        pub(crate) async fn show(context: Context<'_>, name: Option<RoleId>) -> Result<()> {
            let pool = &context.data().pool;

            let team: Team = match name {
                Some(role) => {
                    match Team::get_by_role(pool, role.0).await? {
                        Some(team) => team,
                        None => {
                            context.say(format!("Role <@&{role}> is not associated with a team!")).await?;
                            return Ok(());
                        }
                    }
                },
                None => {
                    match Team::get_by_member(pool, context.author().id.0).await? {
                        Some(team) => team,
                        None => {
                            context.say("You are not on a team!").await?;
                            return Ok(());
                        }
                    }
                }
            };

            let mut members = team.members(pool).await?;
            members.retain(|member| *member != team.capitan);

            let capitan = format!("<@{capitan}>", capitan = team.capitan);
            let members = members
                .into_iter()
                .map(|member| format!("<@{member}>"))
                .collect::<Vec<_>>()
                .join(", ");

            context.say(format!("Team <@&{role}>\n\tCapitan: {capitan}\n\tMembers: {members}", role = team.role)).await?;
            Ok(())
        }

        #[command(slash_command, guild_only, ephemeral)]
        pub(crate) async fn leave(context: Context<'_>) -> Result<()> {
            let pool = &context.data().pool;
            
            let author = context.author().id;
            let guild = context.guild_id().ok_or_else::<Error, _>(|| unreachable!())?;

            // User has team
            let team = match Team::get_by_member(pool, author.0).await? {
                None => {
                    context.say("You are not on a team!").await?;
                    return Ok(());
                },
                Some(team) => team,
            };
            let members = team.members(pool).await?;
            
            // User is not team capitan OR is only member
            if author.0 == team.capitan && [author.0] != members.as_slice() {
                context.say("A capitan cannot leave a team while it has members!").await?;
                return Ok(());
            }

            Team::remove_member(pool, team.id, author.0).await?;
            let mut member = guild.member(context.discord(), author.0).await?;
            member.remove_role(context.discord(), team.role).await?;

            if [author.0] == members.as_slice() {
                Team::delete(pool, team.id).await?;
                guild.delete_role(context.discord(), team.role).await?;
                context.say("Team disbanded.").await?;
            } else {
                context.say("You left the team.").await?;
            }

            Ok(())
        }

        #[command(slash_command, guild_only, ephemeral)]
        pub(crate) async fn invite(context: Context<'_>, user: User) -> Result<()> {
            let pool = &context.data().pool;

            let guild = context.guild_id().ok_or_else::<Error, _>(|| unreachable!())?;
            let author = context.author();

            // Author has team
            let team = match Team::get_by_member(pool, author.id.0).await? {
                None => {
                    context.say("You are not on a team!").await?;
                    return Ok(());
                },
                Some(team) => team,
            };

            // Author is team capitan
            if author.id.0 != team.capitan {
                context.say("You are not the capitan of this team!").await?;
                return Ok(());
            }

            // User does not have team
            if let Some(user_team) = Team::get_by_member(pool, user.id.0).await? {
                if team.id == user_team.id {
                    context.say("This user is already on your team!").await?;
                } else {
                    context.say("This user is already on a team!").await?;
                }
                return Ok(());
            }

            let mut message = user.dm(context.discord(), |message| {
                message
                    .content(format!("You have been invited to join the <@&{}> team by <@{}>!", team.role, author.id))
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
            }).await?;
            let reply = context.say("Invitation sent.").await?;

            let interaction = message
                .await_component_interaction(context.discord())
                .author_id(user.id)
                .await;
            let response = match &interaction {
                Some(interaction) => interaction.data.custom_id.as_str(),
                None => {
                    reply.edit(context, |message| {
                        message.content("Invitation expired.")
                    }).await?;
                    return Ok(());
                }
            };

            reply.edit(context, |reply|
                reply.content(format!("{name} {response} the invitation.", name = user.name))
            ).await?;
            message.edit(context.discord(), |message| {
                message
                    .content(format!("You have {response} the invitation!"))
                    .set_components(Default::default())
            }).await?;

            match response {
                "accepted" => {
                    Team::add_member(pool, team.id, user.id.0).await?;
                    let mut member = guild.member(context.discord(), user.id).await?;
                    member.add_role(context.discord(), team.role).await?;
                },
                "declined" => {},
                _ => unreachable!(),
            }

            Ok(())
        }

        #[command(slash_command, guild_only, ephemeral)]
        pub(crate) async fn kick(context: Context<'_>, user: User) -> Result<()> {
            let pool = &context.data().pool;

            let guild = context.guild_id().ok_or_else::<Error, _>(|| unreachable!())?;
            let author = context.author();

            // User is not Author
            if user.id == author.id {
                context.say("You cannot kick yourself from the team!").await?;
                return Ok(());
            }

            // Author has team
            let team = match Team::get_by_member(pool, author.id.0).await? {
                None => {
                    context.say("You are not on a team!").await?;
                    return Ok(());
                },
                Some(team) => team,
            };

            // Author is team capitan
            if author.id.0 != team.capitan {
                context.say("You are not the capitan of this team!").await?;
                return Ok(());
            }

            // User is on team, and it is author's team
            if let Some(user_team) = Team::get_by_member(pool, user.id.0).await? {
                if user_team.id != team.id {
                    context.say(format!("<@{}> is not on your team!", user.id)).await?;
                    return Ok(());
                }
            } else {
                context.say(format!("<@{}> is not on a team!", user.id)).await?;
                return Ok(());
            }

            Team::remove_member(pool, team.id, user.id.0).await?;

            let mut member = guild.member(context.discord(), user.id).await?;
            member.remove_role(context.discord(), team.role).await?;

            context.say(format!("You kicked <@{}> from the team.", user.id)).await?;
            Ok(())
        }

        #[command(slash_command, guild_only, ephemeral)]
        pub(crate) async fn transfer(context: Context<'_>, user: User) -> Result<()> {
            let pool = &context.data().pool;

            let guild = context.guild_id().ok_or_else::<Error, _>(|| unreachable!())?;
            let author = context.author();

            // User is not Author
            if user.id == author.id {
                context.say("You cannot transfer the team to yourself!").await?;
                return Ok(());
            }

            // Author has team
            let team = match Team::get_by_member(pool, author.id.0).await? {
                None => {
                    context.say("You are not on a team!").await?;
                    return Ok(());
                },
                Some(team) => team,
            };

            // Author is team capitan
            if author.id.0 != team.capitan {
                context.say("You are not the capitan of this team!").await?;
                return Ok(());
            }

            // User is on team, and it is author's team
            if let Some(user_team) = Team::get_by_member(pool, user.id.0).await? {
                if user_team.id != team.id {
                    context.say(format!("<@{}> is not on your team!", user.id)).await?;
                    return Ok(());
                }
            } else {
                context.say(format!("<@{}> is not on a team!", user.id)).await?;
                return Ok(());
            }

            Team::update_capitan(pool, team.id, user.id.0).await?;

            let mut member = guild.member(context.discord(), user.id).await?;
            member.remove_role(context.discord(), team.role).await?;

            context.say(format!("You have transfered the captain position to <@{}>.", user.id)).await?;
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("matchbot", log::LevelFilter::Info)
        .parse_default_env()
        .init();
    
    let pool = PgPool::connect(&var("DATABASE_URL").expect("missing DATABASE_URL")).await.unwrap();
    if let Err(error) = MIGRATOR.run(&pool).await {
        log::error!("Migration error: {}", error);
        std::process::exit(1);
    }

    let framework = Framework::<_, Error>::builder()
        .options(FrameworkOptions {
            commands: vec![commands::admin(), commands::team()],
            listener: move |context, event, framework, _data| Box::pin(async move {
                if let Event::Ready { data_about_bot } = event {
                    let commands_builder = create_application_commands(&framework.options().commands);
                    let commands_count = commands_builder.0.len();
                    for guild in &data_about_bot.guilds {
                        let guild = guild.id.to_partial_guild(context).await?;
                        
                        let commands_builder = commands_builder.clone();
                        guild.id.set_application_commands(context, |builder| {
                            *builder = commands_builder;
                            builder
                        }).await?;
                        
                        log::info!("Registered {} commands for `{}`.", commands_count, guild.name);
                    }
                }
                Ok(())
            }),
            ..Default::default()
        })
        .token(var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN"))
        .intents(GatewayIntents::empty())
        .user_data_setup(move |_context, _ready, _framework| Box::pin(async move {
            Ok(Data {
                pool
            })
        }));

    if let Err(error) = framework.run().await {
        log::error!("Error: {}", error);
    }
}
