use std::env;
use std::str::FromStr;
use diesel::{PgConnection};

use serde::{Deserialize, Serialize};
use serenity::async_trait;
use serenity::Client;
use serenity::client::Context;
use serenity::framework::standard::StandardFramework;


use serenity::model::prelude::GuildId;
use serenity::model::prelude::Ready;
use serenity::prelude::{EventHandler, GatewayIntents, TypeMapKey};

use r2d2::{Pool};
use r2d2_diesel::ConnectionManager;
use serenity::model::application::command::{CommandOptionType};
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::channel::Message;
use match_bot::models::{Match, SeriesType, StepType};

mod commands;
mod utils;
mod dathost_models;

#[derive(Serialize, Deserialize)]
struct Config {
    discord: DiscordConfig,
}

#[derive(Serialize, Deserialize)]
struct DiscordConfig {
    token: String,
    admin_role_id: u64,
    application_id: u64,
    guild_id: u64,
}

#[derive(PartialEq)]
struct StateContainer {
    state: State,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Veto {
    map: String,
    vetoed_by: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStep {
    pub match_id: i32,
    pub step_type: StepType,
    pub team_role_id: i64,
    pub map: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupMap {
    pub match_id: i32,
    pub map: String,
    pub picked_by: i64,
    pub start_attack_team_role_id: Option<i64>,
    pub start_defense_team_role_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setup {
    team_one: Option<i64>,
    team_two: Option<i64>,
    team_one_name: String,
    team_two_name: String,
    team_one_conn_str: Option<String>,
    team_two_conn_str: Option<String>,
    maps_remaining: Vec<String>,
    maps: Vec<SetupMap>,
    vetoes: Vec<Veto>,
    series_type: SeriesType,
    match_id: Option<i32>,
    veto_pick_order: Vec<SetupStep>,
    current_step: usize,
    current_phase: State,
    server_id: Option<String>,
}

#[derive(Debug, Copy, PartialEq, Serialize, Deserialize, Clone)]
pub enum State {
    MapVeto,
    SidePick,
    ServerPick,
}

struct Handler;


struct Maps;

struct Matches;

struct DBConnectionPool;


impl TypeMapKey for Config {
    type Value = Config;
}

impl TypeMapKey for Maps {
    type Value = Vec<String>;
}

impl TypeMapKey for Setup {
    type Value = Setup;
}

impl TypeMapKey for Matches {
    type Value = Vec<Match>;
}

impl TypeMapKey for DBConnectionPool {
    type Value = Pool<ConnectionManager<PgConnection>>;
}

enum Command {
    SteamId,
    Schedule,
    Addmatch,
    Deletematch,
    Match,
    Matches,
    Maps,
}

impl FromStr for Command {
    type Err = ();
    fn from_str(input: &str) -> Result<Command, Self::Err> {
        match input {
            "steamid" => Ok(Command::SteamId),
            "schedule" => Ok(Command::Schedule),
            "addmatch" => Ok(Command::Addmatch),
            "deletematch" => Ok(Command::Deletematch),
            "match" => Ok(Command::Match),
            "matches" => Ok(Command::Matches),
            "maps" => Ok(Command::Maps),
            _ => Err(()),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, context: Context, ready: Ready) {
        let config = load_config().await.unwrap();
        let guild_id = GuildId(config.discord.guild_id);
        let commands = GuildId::set_application_commands(&guild_id, &context.http, |commands| {
            return commands
                .create_application_command(|command| {
                    command.name("maps").description("Lists the current map pool")
                })
                .create_application_command(|command| {
                    command.name("steamid").description("Set your steamID").create_option(|option| {
                        option
                            .name("steamid")
                            .description("Your steamID, i.e. STEAM_0:1:12345678")
                            .kind(CommandOptionType::String)
                            .required(true)
                    })
                })
                .create_application_command(|command| {
                    command.name("match").description("Show match info").create_option(|option| {
                        option
                            .name("matchid")
                            .description("Match ID")
                            .kind(CommandOptionType::String)
                            .required(true)
                    })
                })
                .create_application_command(|command| {
                    command.name("matches").description("Show matches").create_option(|option| {
                        option
                            .name("displayid")
                            .description("Display match IDs")
                            .kind(CommandOptionType::Boolean)
                            .required(false)
                    })
                        .create_option(|option| {
                            option
                                .name("showcompleted")
                                .description("Shows only completed matches")
                                .kind(CommandOptionType::Boolean)
                                .required(false)
                        })
                })
                .create_application_command(|command| {
                    command.name("deletematch").description("Delete match (admin required)").create_option(|option| {
                        option
                            .name("matchid")
                            .description("Match ID")
                            .kind(CommandOptionType::Integer)
                            .required(true)
                    })
                })
                .create_application_command(|command| {
                    command.name("setup").description("Setup your next match")
                })
                .create_application_command(|command| {
                    command.name("addmatch").description("Add match to schedule (admin required)").create_option(|option| {
                        option
                            .name("teamone")
                            .description("Team 1 (Home)")
                            .kind(CommandOptionType::Role)
                            .required(true)
                    }).create_option(|option| {
                        option
                            .name("teamtwo")
                            .description("Team 2 (Away)")
                            .kind(CommandOptionType::Role)
                            .required(true)
                    }).create_option(|option| {
                        option
                            .name("type")
                            .description("Series Type")
                            .kind(CommandOptionType::String)
                            .required(true)
                            .add_string_choice("Best of 1", "bo1")
                            .add_string_choice("Best of 3", "bo3")
                            .add_string_choice("Best of 5", "bo5")
                    }).create_option(|option| {
                        option
                            .name("note")
                            .description("Note")
                            .kind(CommandOptionType::String)
                            .required(false)
                    })
                })
                .create_application_command(|command| {
                    command.name("schedule").description("Schedule your next match").create_option(|option| {
                        option
                            .name("date")
                            .description("Date (Month/Day/Year) @ Time <Timezone>")
                            .kind(CommandOptionType::String)
                            .required(true)
                    })
                })
            ;
        }).await;
        println!("{} is connected!", ready.user.name);
        println!("Added these guild slash commands: {:#?}", commands);
    }
    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(inc_command) = interaction {
            let command = inc_command.data.name.as_str().to_lowercase();
            if let Ok(normal_command) = Command::from_str(&command) {
                let content: String = match normal_command {
                    Command::SteamId => commands::handle_steam_id(&context, &inc_command).await,
                    Command::Addmatch => commands::handle_add_match(&context, &inc_command).await,
                    Command::Deletematch => commands::handle_delete_match(&context, &inc_command).await,
                    Command::Schedule => commands::handle_schedule(&context, &inc_command).await,
                    Command::Match => commands::handle_match(&context, &inc_command).await,
                    Command::Matches => commands::handle_matches(&context, &inc_command).await,
                    Command::Maps => commands::handle_map_list(&context).await,
                };
                if let Err(why) = create_int_resp(&context, &inc_command, content).await {
                    eprintln!("Cannot respond to slash command: {}", why);
                }
            }
            if command == "setup" {
                commands::handle_setup(&context, &inc_command).await;
            }
        }
    }
}

async fn create_int_resp(context: &Context, inc_command: &ApplicationCommandInteraction, content: String) -> serenity::Result<()> {
    return inc_command
        .create_interaction_response(&context.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content(content))
        }).await;
}

#[tokio::main]
async fn main() {
    let config = load_config().await.unwrap();
    let token = &config.discord.token;
    let framework = StandardFramework::new();
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {})
        .framework(framework)
        .application_id(config.discord.application_id)
        .await
        .expect("Error creating client");
    {
        let mut data = client.data.write().await;
        data.insert::<Config>(config);
        data.insert::<DBConnectionPool>(get_connection_pool());
    }
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

pub fn get_connection_pool() -> Pool<ConnectionManager<PgConnection>> {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .max_size(15)
        .build(manager)
        .expect("Could not build connection pool")
}

async fn load_config() -> Result<Config, serde_yaml::Error> {
    let config: Config = Config {
        discord: DiscordConfig {
            token: option_env!("DISCORD_TOKEN").expect("DISCORD_TOKEN not defined").to_string(),
            admin_role_id: option_env!("DISCORD_ADMIN_ROLE_ID").expect("DISCORD_ADMIN_ROLE_ID not defined").parse().unwrap(),
            application_id: option_env!("DISCORD_APPLICATION_ID").expect("DISCORD_APPLICATION_ID not defined").parse().unwrap(),
            guild_id: option_env!("DISCORD_GUILD_ID").expect("DISCORD_GUILD_ID not defined").parse().unwrap(),
        }
    };
    Ok(config)
}
