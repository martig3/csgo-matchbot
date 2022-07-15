use std::collections::HashMap;
use std::str::FromStr;
use chrono::{DateTime, NaiveDate, Utc};


use serde::{Deserialize, Serialize};
use serenity::async_trait;
use serenity::Client;
use serenity::client::Context;
use serenity::framework::standard::StandardFramework;
use serenity::model::guild::Role;
use serenity::model::prelude::{GuildId, Interaction, InteractionResponseType, Ready, RoleId};
use serenity::model::prelude::application_command::{ApplicationCommandInteraction, ApplicationCommandOptionType};
use serenity::prelude::{EventHandler, TypeMapKey};
use uuid::Uuid;
use crate::SeriesType::{Bo1, Bo3, Bo5};

mod commands;
mod utils;

#[derive(Serialize, Deserialize)]
struct Config {
    discord: DiscordConfig,
}

#[derive(Serialize, Deserialize)]
struct DiscordConfig {
    token: String,
    admin_role_id: Option<u64>,
    application_id: u64,
    guild_id: u64,
}

#[derive(PartialEq)]
struct StateContainer {
    state: State,
}

#[derive(Clone, Serialize, Deserialize)]
struct SeriesMap {
    map: String,
    picked_by: RolePartial,
    start_attack: Option<RolePartial>,
    start_defense: Option<RolePartial>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Veto {
    map: String,
    vetoed_by: Role,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
enum MatchState {
    Entered,
    Scheduled,
    Completed,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct RolePartial {
    id: RoleId,
    name: String,
    guild_id: GuildId,
}

#[derive(Clone, Serialize, Deserialize)]
struct ScheduleInfo {
    date: NaiveDate,
    time_str: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct SetupInfo {
    series_type: SeriesType,
    maps: Vec<SeriesMap>,
    vetos: Vec<SetupStep>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Match {
    id: Uuid,
    team_one: RolePartial,
    team_two: RolePartial,
    note: Option<String>,
    date_added: DateTime<Utc>,
    match_state: MatchState,
    schedule_info: Option<ScheduleInfo>,
    setup_info: Option<SetupInfo>,
}

#[derive(Clone, Serialize, Deserialize)]
enum SeriesType {
    Bo1,
    Bo3,
    Bo5,
}

#[derive(PartialEq, Clone, Serialize, Deserialize)]
enum StepType {
    Veto,
    Pick,
}

#[derive(Clone, Serialize, Deserialize)]
struct SetupStep {
    step_type: StepType,
    team: RolePartial,
    map: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Setup {
    team_one: Option<RolePartial>,
    team_two: Option<RolePartial>,
    maps_remaining: Vec<String>,
    maps: Vec<SeriesMap>,
    vetoes: Vec<Veto>,
    series_type: SeriesType,
    match_id: Option<Uuid>,
    veto_pick_order: Vec<SetupStep>,
    current_step: usize,
    current_phase: State,
}

#[derive(PartialEq, Serialize, Deserialize, Clone)]
enum State {
    Idle,
    MapVeto,
    SidePick,
    Setup,
}

struct Handler;

struct RiotIdCache;

struct BotState;

struct Maps;

struct Matches;


impl TypeMapKey for Config {
    type Value = Config;
}

impl TypeMapKey for RiotIdCache {
    type Value = HashMap<u64, String>;
}

impl TypeMapKey for BotState {
    type Value = StateContainer;
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

enum Command {
    Setup,
    Schedule,
    Addmatch,
    Deletematch,
    Match,
    Matches,
    Maps,
    Cancel,
    Defense,
    Attack,
    Pick,
    Ban,
    Help,
}

impl FromStr for SeriesType {
    type Err = ();
    fn from_str(input: &str) -> Result<SeriesType, Self::Err> {
        match input {
            "bo1" => Ok(Bo1),
            "bo3" => Ok(Bo3),
            "bo5" => Ok(Bo5),
            _ => Err(()),
        }
    }
}

impl ToString for StepType {
    fn to_string(&self) -> String {
        String::from(match &self {
            StepType::Veto => "/ban",
            StepType::Pick => "/pick",
        })
    }
}

impl FromStr for Command {
    type Err = ();
    fn from_str(input: &str) -> Result<Command, Self::Err> {
        match input {
            "setup" => Ok(Command::Setup),
            "schedule" => Ok(Command::Schedule),
            "addmatch" => Ok(Command::Addmatch),
            "deletematch" => Ok(Command::Deletematch),
            "match" => Ok(Command::Match),
            "matches" => Ok(Command::Matches),
            "maps" => Ok(Command::Maps),
            "cancel" => Ok(Command::Cancel),
            "defense" => Ok(Command::Defense),
            "attack" => Ok(Command::Attack),
            "pick" => Ok(Command::Pick),
            "ban" => Ok(Command::Ban),
            "help" => Ok(Command::Help),
            _ => Err(()),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, context: Context, ready: Ready) {
        let config = read_config().await.unwrap();
        let guild_id = GuildId(config.discord.guild_id);
        let commands = GuildId::set_application_commands(&guild_id, &context.http, |commands| {
            return commands
                .create_application_command(|command| {
                    command.name("maps").description("Lists the current map pool")
                })
                .create_application_command(|command| {
                    command.name("cancel").description("Cancels setup (requires admin)")
                })
                .create_application_command(|command| {
                    command.name("attack").description("Select attack starting side")
                })
                .create_application_command(|command| {
                    command.name("defense").description("Select defense starting side")
                })
                .create_application_command(|command| {
                    command.name("help").description("DM yourself help info")
                })
                .create_application_command(|command| {
                    command.name("match").description("Show matches").create_option(|option| {
                        option
                            .name("matchid")
                            .description("Match ID")
                            .kind(ApplicationCommandOptionType::String)
                            .required(true)
                    })
                })
                .create_application_command(|command| {
                    command.name("matches").description("Show matches").create_option(|option| {
                        option
                            .name("displayid")
                            .description("Display match IDs")
                            .kind(ApplicationCommandOptionType::Boolean)
                            .required(false)
                    })
                        .create_option(|option| {
                            option
                                .name("showcompleted")
                                .description("Shows only completed matches")
                                .kind(ApplicationCommandOptionType::Boolean)
                                .required(false)
                        })
                })
                .create_application_command(|command| {
                    command.name("deletematch").description("Delete match (admin required)").create_option(|option| {
                        option
                            .name("matchid")
                            .description("Match ID")
                            .kind(ApplicationCommandOptionType::String)
                            .required(true)
                    })
                })
                .create_application_command(|command| {
                    command.name("setup").description("Setup your next match").create_option(|option| {
                        option
                            .name("type")
                            .description("Series Type")
                            .kind(ApplicationCommandOptionType::String)
                            .required(true)
                            .add_string_choice("Best of 1", "bo1")
                            .add_string_choice("Best of 3", "bo3")
                            .add_string_choice("Best of 5", "bo5")
                    })
                })
                .create_application_command(|command| {
                    command.name("pick").description("Pick a map during the map veto").create_option(|option| {
                        option
                            .name("map")
                            .description("Map name")
                            .kind(ApplicationCommandOptionType::String)
                            .required(true)
                            .add_string_choice("Ascent", "ascent")
                            .add_string_choice("Bind", "bind")
                            .add_string_choice("Breeze", "breeze")
                            .add_string_choice("Fracture", "fracture")
                            .add_string_choice("Haven", "haven")
                            .add_string_choice("Icebox", "icebox")
                            .add_string_choice("Split", "split")
                    })
                })
                .create_application_command(|command| {
                    command.name("ban").description("Ban a map during the map veto").create_option(|option| {
                        option
                            .name("map")
                            .description("Map name")
                            .kind(ApplicationCommandOptionType::String)
                            .required(true)
                            .add_string_choice("Ascent", "ascent")
                            .add_string_choice("Bind", "bind")
                            .add_string_choice("Breeze", "breeze")
                            .add_string_choice("Fracture", "fracture")
                            .add_string_choice("Haven", "haven")
                            .add_string_choice("Icebox", "icebox")
                            .add_string_choice("Split", "split")
                    })
                })
                .create_application_command(|command| {
                    command.name("addmatch").description("Add match to schedule (admin required)").create_option(|option| {
                        option
                            .name("teamone")
                            .description("Team 1 (Home)")
                            .kind(ApplicationCommandOptionType::Role)
                            .required(true)
                    }).create_option(|option| {
                        option
                            .name("teamtwo")
                            .description("Team 2 (Away)")
                            .kind(ApplicationCommandOptionType::Role)
                            .required(true)
                    }).create_option(|option| {
                        option
                            .name("note")
                            .description("Note")
                            .kind(ApplicationCommandOptionType::String)
                            .required(false)
                    })
                })
                .create_application_command(|command| {
                    command.name("schedule").description("Schedule your next match").create_option(|option| {
                        option
                            .name("date")
                            .description("Date (Month/Day/Year)")
                            .kind(ApplicationCommandOptionType::String)
                            .required(true)
                    }).create_option(|option| {
                        option
                            .name("time")
                            .description("Time (include timezone) i.e. 10EST")
                            .kind(ApplicationCommandOptionType::String)
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
            let command = Command::from_str(&inc_command.data.name.as_str().to_lowercase()).expect("Expected valid command");
            let content: String = match command {
                Command::Setup => commands::handle_setup(&context, &inc_command).await,
                Command::Addmatch => commands::handle_add_match(&context, &inc_command).await,
                Command::Deletematch => commands::handle_delete_match(&context, &inc_command).await,
                Command::Schedule => commands::handle_schedule(&context, &inc_command).await,
                Command::Match => commands::handle_match(&context, &inc_command).await,
                Command::Matches => commands::handle_matches(&context, &inc_command).await,
                Command::Maps => commands::handle_map_list(&context).await,
                Command::Defense => commands::handle_defense_option(&context, &inc_command).await,
                Command::Attack => commands::handle_attack_option(&context, &inc_command).await,
                Command::Pick => commands::handle_pick_option(&context, &inc_command).await,
                Command::Ban => commands::handle_ban_option(&context, &inc_command).await,
                Command::Cancel => commands::handle_cancel(&context, &inc_command).await,
                Command::Help => commands::handle_help(&context, &inc_command).await,
            };
            if let Err(why) = create_int_resp(&context, &inc_command, content).await {
                eprintln!("Cannot respond to slash command: {}", why);
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
    let config = read_config().await.unwrap();
    let token = &config.discord.token;
    let framework = StandardFramework::new();
    let mut client = Client::builder(&token)
        .event_handler(Handler {})
        .framework(framework)
        .application_id(config.discord.application_id)
        .await
        .expect("Error creating client");
    {
        let mut data = client.data.write().await;
        data.insert::<Config>(config);
        data.insert::<RiotIdCache>(read_riot_ids().await.unwrap());
        data.insert::<BotState>(StateContainer { state: State::Idle });
        data.insert::<Maps>(read_maps().await.unwrap());
        data.insert::<Matches>(read_matches().await.unwrap());
        data.insert::<Setup>(Setup {
            team_one: None,
            team_two: None,
            maps: Vec::new(),
            vetoes: Vec::new(),
            maps_remaining: read_maps().await.unwrap(),
            series_type: Bo3,
            match_id: None,
            veto_pick_order: Vec::new(),
            current_step: 0,
            current_phase: State::Idle,
        });
    }
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

async fn read_config() -> Result<Config, serde_yaml::Error> {
    let yaml = std::fs::read_to_string("config.yaml").unwrap();
    let config: Config = serde_yaml::from_str(&yaml)?;
    Ok(config)
}

async fn read_riot_ids() -> Result<HashMap<u64, String>, serde_json::Error> {
    if std::fs::read("riot_ids.json").is_ok() {
        let json_str = std::fs::read_to_string("riot_ids.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(HashMap::new())
    }
}

async fn read_maps() -> Result<Vec<String>, serde_json::Error> {
    if std::fs::read("maps.json").is_ok() {
        let json_str = std::fs::read_to_string("maps.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(Vec::new())
    }
}

async fn read_matches() -> Result<Vec<Match>, serde_json::Error> {
    if std::fs::read("matches.json").is_ok() {
        let json_str = std::fs::read_to_string("matches.json").unwrap();
        let json = serde_json::from_str(&json_str).unwrap();
        Ok(json)
    } else {
        Ok(Vec::new())
    }
}


