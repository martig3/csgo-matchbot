mod commands;

use crate::commands::admin::admin;
use crate::commands::matches::matches;
use crate::commands::setup::setup;
use crate::commands::steamid::steamid;
use crate::commands::team::team;
use anyhow::Error;
use dotenvy::{dotenv, var};
use poise::{builtins::create_application_commands, Event, Framework, FrameworkOptions};
use serenity::model::gateway::GatewayIntents;
use sqlx::{migrate::Migrator, PgPool};

static MIGRATOR: Migrator = sqlx::migrate!();

pub struct Data {
    pub pool: PgPool,
}

type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("csgo-matchbot", log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let pool = PgPool::connect(&var("DATABASE_URL").expect("missing DATABASE_URL"))
        .await
        .unwrap();
    if let Err(error) = MIGRATOR.run(&pool).await {
        log::error!("Migration error: {}", error);
        std::process::exit(1);
    }

    let framework = Framework::<_, Error>::builder()
        .options(FrameworkOptions {
            commands: vec![admin(), team(), steamid(), matches(), setup()],
            listener: move |context, event, framework, _data| {
                Box::pin(async move {
                    if let Event::Ready { data_about_bot } = event {
                        let commands_builder =
                            create_application_commands(&framework.options().commands);
                        let commands_count = commands_builder.0.len();
                        for guild in &data_about_bot.guilds {
                            let guild = guild.id.to_partial_guild(&context).await?;

                            let commands_builder = commands_builder.clone();
                            guild
                                .id
                                .set_application_commands(&context, |builder| {
                                    *builder = commands_builder;
                                    builder
                                })
                                .await?;

                            log::info!(
                                "Registered {} commands for `{}`.",
                                commands_count,
                                guild.name
                            );
                        }
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .token(var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN"))
        .intents(GatewayIntents::empty())
        .user_data_setup(move |_context, _ready, _framework| {
            Box::pin(async move { Ok(Data { pool }) })
        });

    if let Err(error) = framework.run().await {
        log::error!("Error: {}", error);
    }
}
