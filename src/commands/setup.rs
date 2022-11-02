use crate::commands::matches::SeriesType::{Bo1, Bo3, Bo5};
use crate::Context;
use anyhow::{Error, Result};
use poise::command;
use poise::futures_util::StreamExt;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serenity::builder::{CreateActionRow, CreateButton, CreateSelectMenu, CreateSelectMenuOption};
use serenity::model::application::component::ButtonStyle;
use serenity::model::application::interaction::message_component::MessageComponentInteraction;
use serenity::model::channel::{ChannelType, ReactionType};
use std::collections::HashMap;
use std::env;

use std::sync::Arc;
use std::time::Duration;

use serenity::model::prelude::interaction::InteractionResponseType;

use crate::commands::admin::ServerTemplates;
use crate::commands::maps::Map;
use crate::commands::matches::VoteType::{Pick, Veto};
use crate::commands::matches::{
    Match, MatchScore, MatchSeries, NewMatch, SeriesType, Server, VoteInfo, VoteType,
};
use crate::commands::steamid::SteamUser;
use crate::commands::team::Team;

use sqlx::PgPool;
use steamid::{SteamId, Universe};
use urlencoding::encode;

#[derive(Debug, Copy, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum SetupState {
    MapVeto,
    SidePick,
    ServerPick,
}
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DathostStartResponse {
    id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerDuplicateResponse {
    pub game: Option<String>,
    pub id: String,
    pub ip: String,
    pub ports: Ports,
    pub location: Option<String>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ports {
    pub game: i64,
    pub gotv: i64,
}

#[derive(Debug, Copy, Clone)]
pub struct NewVoteInfo {
    pub match_series: i32,
    pub map: Option<i32>,
    pub vote_type: VoteType,
    pub team_role: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DathostConfig {
    pub user: String,
    pub password: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CreateGsltRequest {
    pub key: String,
    pub appid: u32,
    pub memo: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteamApiRootResponse {
    pub response: SteamApiResponse,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteamApiResponse {
    pub steamid: String,
    pub login_token: String,
}

#[derive(Debug, Clone)]
pub struct Setup {
    team_one_conn_str: Option<String>,
    team_two_conn_str: Option<String>,
    maps_remaining: Vec<String>,
    maps_sel: Vec<NewMatch>,
    series_type: SeriesType,
    match_series: Option<i32>,
    veto_pick_order: Vec<NewVoteInfo>,
    current_step: usize,
    current_phase: SetupState,
    servers_remaining: Vec<ServerTemplates>,
    server_veto_team: i64,
    server_id: Option<String>,
    server_hostname: Option<String>,
    server_game_port: Option<i64>,
    server_gotv_port: Option<i64>,
    team_one: Team,
    team_two: Team,
}

impl Setup {
    async fn finish(&self, executor: &PgPool) -> Result<()> {
        for vote_info in &self.veto_pick_order {
            let team = if vote_info.team_role == self.team_one.role {
                self.team_one.id
            } else {
                self.team_two.id
            };
            VoteInfo::add(
                executor,
                self.match_series.unwrap(),
                vote_info.map.unwrap(),
                vote_info.vote_type,
                team,
            )
            .await?;
        }
        for map in &self.maps_sel {
            let picked_by = if map.picked_by_role == self.team_one.role {
                self.team_one.id
            } else {
                self.team_two.id
            };
            let start_ct_team = if map.start_ct_team_role.unwrap() == self.team_one.role {
                self.team_one.id
            } else {
                self.team_two.id
            };
            let start_t_team = if map.start_t_team_role.unwrap() == self.team_one.role {
                self.team_one.id
            } else {
                self.team_two.id
            };
            let m = Match::create(
                executor,
                self.match_series.unwrap(),
                map.map_id,
                picked_by,
                start_ct_team,
                start_t_team,
            )
            .await?;
            MatchScore::add(executor, m.id).await?;
        }
        Ok(())
    }
}

async fn bo1_setup(match_series: i32, team_one: i64, team_two: i64) -> (Vec<NewVoteInfo>, String) {
    (
        vec![
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_one,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_one,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_one,
                map: None,
            },
        ],
        format!(
            "Best of 1 option selected. Starting map veto. <@&{}> bans first.\n",
            &team_two
        ),
    )
}

async fn bo3_setup(match_series: i32, team_one: i64, team_two: i64) -> (Vec<NewVoteInfo>, String) {
    (
        vec![
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_one,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_one,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_one,
                map: None,
            },
        ],
        format!(
            "Best of 3 option selected. Starting map veto. <@&{}> bans first.\n",
            &team_one
        ),
    )
}

async fn bo5_setup(match_series: i32, team_one: i64, team_two: i64) -> (Vec<NewVoteInfo>, String) {
    (
        vec![
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_one,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Veto,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_one,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_one,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_two,
                map: None,
            },
            NewVoteInfo {
                match_series,
                vote_type: Pick,
                team_role: team_one,
                map: None,
            },
        ],
        format!(
            "Best of 5 option selected. Starting map veto. <@&{}> bans first.\n",
            &team_one
        ),
    )
}

#[command(
    slash_command,
    guild_only,
    description_localized("en-US", "Setup your next scheduled match")
)]
pub(crate) async fn setup(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;
    let current_match = MatchSeries::next_user_match(pool, context.author().id.0 as i64).await;
    if current_match.is_err() {
        log::error!("{:#?}", current_match.err().unwrap());
        context.say("No scheduled matches found").await?;
        return Ok(());
    }
    let mut current_match = current_match.unwrap();
    if current_match.dathost_match.is_some() {
        context
            .say("Your next match is already setup and in progress.")
            .await?;
        return Ok(());
    }
    let maps = Map::get_all(pool, true).await?;
    let maps_names: Vec<String> = maps
        .clone()
        .into_iter()
        .filter(|m| !m.disabled)
        .map(|m| m.name)
        .collect();
    if maps_names.len() < 7 {
        context
            .say("At least 7 maps need to be enabled before starting setup.")
            .await?;
        return Ok(());
    }
    let team_one = Team::get(pool, current_match.team_one).await?;
    let team_two = Team::get(pool, current_match.team_two).await?;
    let series_setup = match current_match.series_type {
        Bo1 => bo1_setup(current_match.id, team_one.role, team_two.role).await,
        Bo3 => bo3_setup(current_match.id, team_one.role, team_two.role).await,
        Bo5 => bo5_setup(current_match.id, team_one.role, team_two.role).await,
    };
    let servers_remaining = ServerTemplates::get_all(pool).await?;
    if servers_remaining.len() == 0 {
        context
            .say("No server templates have been added, use `/admin servers add` to add some.")
            .await?;
        return Ok(());
    }
    let mut setup: Setup = Setup {
        maps_remaining: maps_names,
        maps_sel: vec![],
        series_type: current_match.series_type,
        match_series: Some(current_match.id),
        veto_pick_order: series_setup.0,
        current_step: 0,
        current_phase: SetupState::ServerPick,
        server_id: None,
        server_veto_team: team_two.clone().role,
        team_one,
        team_two,
        servers_remaining,
        server_hostname: None,
        server_game_port: None,
        team_two_conn_str: None,
        team_one_conn_str: None,
        server_gotv_port: None,
    };
    let m = context
        .say(format!(
            "Starting setup for <@&{}> vs <@&{}> ⤵️",
            setup.team_one.role, setup.team_two.role
        ))
        .await?;
    let thread = context
        .channel_id()
        .create_public_thread(context.discord(), m.message().await?.id, |t| {
            t.kind(ChannelType::PublicThread);
            t.name(format!(
                "Match Setup - {} vs {} ",
                setup.team_one.name, setup.team_two.name
            ));
            t
        })
        .await?;
    let mut m = thread.say(context.discord(), "Starting setup...").await?;
    if setup.servers_remaining.len() > 2 {
        m.edit(context.discord(), |d| {
            d.content(format!(
                "\nIt is <@&{}> turn to ban a server",
                setup.team_two.role
            ))
            .components(|c| {
                c.add_action_row(create_server_action_row(
                    setup.servers_remaining.clone(),
                    &Veto,
                ))
            })
        })
        .await?;
    } else {
        m.edit(context.discord(), |d| {
            d.content(format!(
                "\nIt is <@&{}> turn to pick a server",
                setup.team_two.role
            ))
            .components(|c| {
                c.add_action_row(create_server_action_row(
                    setup.servers_remaining.clone(),
                    &Pick,
                ))
            })
        })
        .await?;
    }
    let mut cib = m.await_component_interactions(&context.discord()).build();
    while let Some(mci) = cib.next().await {
        let completed = match setup.current_phase {
            SetupState::ServerPick => {
                server_pick_phase(pool, &context, &mci, &mut setup, &series_setup.1).await?
            }
            SetupState::MapVeto => {
                map_veto_phase(pool, &context, &mci, &mut setup, &maps, &current_match).await?
            }
            SetupState::SidePick => {
                side_pick_phase(pool, &context, &mci, &mut setup, &maps).await?
            }
        };
        if completed {
            match start_server(&context, pool, &mci, &mut setup, &mut current_match).await {
                Ok(resp) => {
                    setup.finish(pool).await?;
                    send_conn_msg(&context, pool, &mci, &setup, resp).await;
                    return Ok(());
                }
                Err(err) => {
                    eprintln!("{:#?}", err)
                }
            }
            return Ok(());
        }
    }
    Ok(())
}

async fn server_pick_phase(
    pool: &PgPool,
    context: &Context<'_>,
    mci: &Arc<MessageComponentInteraction>,
    setup: &mut Setup,
    init_veto_msg: &String,
) -> Result<bool> {
    let t = Team::get_by_member(pool, mci.user.id.0 as i64).await;
    if let Err(_err) = &t {
        no_team_resp(context, &mci).await;
        return Ok(false);
    }
    match t.unwrap() {
        Some(team) => {
            if setup.server_veto_team != team.role {
                mci.create_interaction_response(&context.discord(), |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.ephemeral(true)
                                .content("It is not your team's turn to pick or ban a server")
                        })
                })
                .await
                .unwrap();
                return Ok(false);
            }
            let choice_loc = mci.data.values.get(0).unwrap();
            if setup.servers_remaining.len() > 2 {
                let pos_remove = setup
                    .servers_remaining
                    .iter()
                    .position(|s| &s.location == choice_loc)
                    .unwrap();
                let previous_step = if setup.servers_remaining.len() > 2 {
                    Veto
                } else {
                    Pick
                };
                setup.servers_remaining.remove(pos_remove);
                let current_step = if setup.servers_remaining.len() > 2 {
                    Veto
                } else {
                    Pick
                };
                let next_team = if setup.server_veto_team == setup.team_one.role {
                    setup.team_two.role
                } else {
                    setup.team_one.role
                };
                let content = format!(
                    "<@&{}> {} `{}`, <@&{}> turn to {} a server",
                    setup.server_veto_team, previous_step, choice_loc, next_team, current_step
                );
                setup.server_veto_team = next_team;
                mci.create_interaction_response(&context.discord(), |r| {
                    r.kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|d| {
                            d.content(content).components(|c| {
                                c.add_action_row(create_server_action_row(
                                    setup.servers_remaining.clone(),
                                    &current_step,
                                ))
                            })
                        })
                })
                .await
                .unwrap();
                return Ok(false);
            }

            let server_id = &setup
                .servers_remaining
                .iter()
                .find(|s| &s.location == choice_loc)
                .unwrap()
                .server_id;
            setup.server_id = Some(String::from(server_id));
            setup.current_phase = SetupState::MapVeto;
            let content = format!(
                "<@&{}> picked `{}`, server pick phase completed.\n{}",
                setup.server_veto_team, choice_loc, init_veto_msg
            );
            mci.create_interaction_response(&context.discord(), |r| {
                r.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|d| {
                        d.content(content).components(|c| {
                            c.add_action_row(create_map_action_row(
                                setup.maps_remaining.clone(),
                                &setup.veto_pick_order[0].vote_type,
                            ))
                        })
                    })
            })
            .await
            .unwrap();
        }
        None => {
            no_team_resp(context, &mci).await;
        }
    }
    Ok(false)
}

async fn side_pick_phase(
    pool: &PgPool,
    context: &Context<'_>,
    mci: &Arc<MessageComponentInteraction>,
    setup: &mut Setup,
    maps: &Vec<Map>,
) -> Result<bool> {
    let option_selected = mci.data.values.get(0).unwrap();
    let t = Team::get_by_member(pool, mci.user.id.0 as i64).await;
    if let Err(_err) = &t {
        no_team_resp(context, &mci).await;
        return Ok(false);
    }
    match t.unwrap() {
        Some(team) => {
            let picked_by = setup.maps_sel[setup.current_step].picked_by_role;
            let not_picked_by = if picked_by == setup.team_one.role {
                setup.team_two.role
            } else {
                setup.team_one.role
            };
            if picked_by == team.role {
                mci.create_interaction_response(&context.discord(), |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.ephemeral(true)
                                .content("It is not your team's turn to pick sides")
                        })
                })
                .await
                .unwrap();
                return Ok(false);
            }
            if setup.maps_sel.len() != setup.current_step + 1 {
                let next_team_picked_by = &setup
                    .maps_sel
                    .get(setup.current_step + 1)
                    .unwrap()
                    .picked_by_role;
                let next_team = if next_team_picked_by == &(setup.team_one.role) {
                    setup.team_two.role
                } else {
                    setup.team_one.role
                };
                let next_map = &setup.maps_sel.get(setup.current_step + 1).unwrap().map_id;
                let next_map_name = &maps.iter().find(|m| &m.id == next_map).unwrap().name;
                mci.create_interaction_response(&context.discord(), |r| {
                    r.kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|d| {
                            d.content(format!(
                                "It is <@&{}> turn to pick starting side on `{}`",
                                next_team, next_map_name
                            ))
                            .components(|c| c.add_action_row(create_sidepick_action_row()))
                        })
                })
                .await
                .unwrap();
            }
            if option_selected == &String::from("ct") {
                setup.maps_sel[setup.current_step].start_ct_team_role = Some(not_picked_by);
                setup.maps_sel[setup.current_step].start_t_team_role = Some(picked_by);
            } else {
                setup.maps_sel[setup.current_step].start_t_team_role = Some(not_picked_by);
                setup.maps_sel[setup.current_step].start_ct_team_role = Some(picked_by);
            }
            setup.current_step += 1;
            if setup.maps_sel.len() == setup.current_step {
                return Ok(true);
            }
        }
        None => {
            no_team_resp(context, &mci).await;
        }
    }
    Ok(false)
}

pub async fn send_conn_msg(
    context: &Context<'_>,
    pool: &PgPool,
    msg: &Arc<MessageComponentInteraction>,
    setup: &Setup,
    server: ServerDuplicateResponse,
) {
    let client = Client::new();
    let game_url = format!("{}:{}", server.ip, server.ports.game);
    let gotv_url = format!("{}:{}", server.ip, server.ports.gotv);
    let url_link = format!("steam://connect/{}", &game_url);
    let gotv_link = format!("steam://connect/{}", &gotv_url);
    let resp = client
        .get(format!(
            "https://tinyurl.com/api-create.php?url={}",
            encode(&url_link)
        ))
        .send()
        .await
        .unwrap();
    let t_url = resp.text_with_charset("utf-8").await.unwrap();
    let resp = client
        .get(format!(
            "https://tinyurl.com/api-create.php?url={}",
            encode(&gotv_link)
        ))
        .send()
        .await
        .unwrap();
    let t_gotv_url = resp.text_with_charset("utf-8").await.unwrap();

    let eos = eos_str(pool, &setup).await.unwrap();
    let mut m = msg
        .channel_id
        .send_message(&context.discord(), |m| {
            m.content(eos).components(|c| {
                c.add_action_row(create_server_conn_button_row(&t_url, &t_gotv_url, true))
            })
        })
        .await
        .unwrap();
    let mut cib = m
        .await_component_interactions(&context.discord())
        .timeout(Duration::from_secs(60 * 5))
        .build();
    loop {
        let opt = cib.next().await;
        match opt {
            Some(mci) => {
                mci.create_interaction_response(&context.discord(), |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.ephemeral(true).content(format!(
                                "Console: ||`connect {}`||\nGOTV: ||`connect {}`||",
                                &game_url, &gotv_url
                            ))
                        })
                })
                .await
                .unwrap();
            }
            None => {
                // remove console cmds interaction on timeout
                let eos = eos_str(pool, &setup).await.unwrap();
                m.edit(&context.discord(), |m| {
                    m.content(eos).components(|c| {
                        c.add_action_row(create_server_conn_button_row(&t_url, &t_gotv_url, false))
                    })
                })
                .await
                .unwrap();
                break;
            }
        }
    }
}

pub fn create_server_conn_button_row(
    url: &String,
    gotv_url: &String,
    show_cmds: bool,
) -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut conn_button = CreateButton::default();
    conn_button.label("Connect");
    conn_button.style(ButtonStyle::Link);
    conn_button.emoji(ReactionType::Unicode("🛰".parse().unwrap()));
    conn_button.url(&url);
    ar.add_button(conn_button);
    if show_cmds {
        let mut console_button = CreateButton::default();
        console_button.custom_id("console");
        console_button.label("Console Cmds");
        console_button.style(ButtonStyle::Secondary);
        console_button.emoji(ReactionType::Unicode("🧾".parse().unwrap()));
        ar.add_button(console_button);
    }
    let mut gotv_button = CreateButton::default();
    gotv_button.label("GOTV");
    gotv_button.style(ButtonStyle::Link);
    gotv_button.emoji(ReactionType::Unicode("📺".parse().unwrap()));
    gotv_button.url(gotv_url);
    ar.add_button(gotv_button);
    ar
}

async fn map_veto_phase(
    pool: &PgPool,
    context: &Context<'_>,
    mci: &Arc<MessageComponentInteraction>,
    setup: &mut Setup,
    maps: &Vec<Map>,
    curr_series: &MatchSeries,
) -> Result<bool> {
    let map_selected = mci.data.values.get(0).unwrap();
    let t = Team::get_by_member(pool, mci.user.id.0 as i64).await;
    if let Err(_err) = &t {
        no_team_resp(context, &mci).await;
        return Ok(false);
    }
    match t.unwrap() {
        Some(team) => {
            let curr_step_info = setup.veto_pick_order.get(setup.current_step).unwrap();
            if curr_step_info.team_role != team.role {
                mci.create_interaction_response(&context.discord(), |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.ephemeral(true)
                                .content("It is not your team's turn to pick or ban")
                        })
                })
                .await
                .unwrap();
                return Ok(false);
            }

            let selected_map_id = maps.iter().find(|m| &m.name == map_selected).unwrap().id;
            if setup.veto_pick_order[setup.current_step].vote_type == Pick {
                setup.maps_sel.push(NewMatch {
                    map_id: selected_map_id,
                    picked_by_role: curr_step_info.team_role,
                    start_t_team_role: None,
                    start_ct_team_role: None,
                })
            }
            setup.veto_pick_order[setup.current_step].map = Some(selected_map_id);

            if setup.veto_pick_order.len() == setup.current_step + 1 {
                let first_map = setup.maps_sel.get(0).unwrap();
                let other_role_id = if setup.maps_sel[0].picked_by_role == setup.team_one.role {
                    setup.team_two.role
                } else {
                    setup.team_one.role
                };
                let next_map_name = &maps.iter().find(|m| m.id == first_map.map_id).unwrap().name;
                mci.create_interaction_response(&context.discord(), |r| {
                    r.kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|d| {
                            d.content(format!(
                                "Map veto completed.\nIt is <@&{}> turn to pick starting side for `{}`",
                                other_role_id, next_map_name
                            ))
                                .components(|c| c.add_action_row(create_sidepick_action_row()))
                        })
                })
                    .await
                    .unwrap();
                setup.current_step = 0;
                setup.current_phase = SetupState::SidePick;
                return Ok(false);
            }

            let next_vote_type = setup.veto_pick_order[setup.current_step + 1].vote_type;
            let next_role_id = setup
                .veto_pick_order
                .get(setup.current_step + 1)
                .unwrap()
                .team_role;
            let map_index = setup
                .maps_remaining
                .iter()
                .position(|m| m == map_selected)
                .unwrap();
            setup.maps_remaining.remove(map_index);
            let curr_vote_info: Vec<VoteInfo> = setup
                .veto_pick_order
                .clone()
                .into_iter()
                .map(|v| VoteInfo {
                    id: 0,
                    match_series: curr_series.id,
                    map: if v.map.is_some() { v.map.unwrap() } else { -1 },
                    vote_type: v.vote_type,
                    team: if v.team_role == setup.team_one.role {
                        setup.team_one.id
                    } else {
                        setup.team_two.id
                    },
                })
                .collect();
            let info_str = curr_series.veto_info(pool, Some(curr_vote_info)).await?;
            mci.create_interaction_response(&context.discord(), |r| {
                r.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|d| {
                        d.content(format!(
                            "{}\nIt is <@&{}> turn to {}",
                            info_str,
                            next_role_id,
                            &next_vote_type.to_string()
                        ))
                        .components(|c| {
                            c.add_action_row(create_map_action_row(
                                setup.maps_remaining.clone(),
                                &next_vote_type,
                            ))
                        })
                    })
            })
            .await
            .unwrap();
            setup.current_step += 1;
            return Ok(false);
        }
        None => {
            no_team_resp(context, &mci).await;
        }
    }
    Ok(false)
}

pub(crate) async fn eos_str(pool: &PgPool, setup: &Setup) -> Result<String> {
    let mut resp = String::from("\n\nSetup is completed. GLHF!\n\n");
    let maps = Map::get_all(pool, true).await?;
    for (i, el) in setup.maps_sel.iter().enumerate() {
        resp.push_str(
            format!(
                "**{}. {}** - picked by: <@&{}>\n",
                i + 1,
                maps.iter().find(|m| m.id == el.map_id).unwrap().name,
                &el.picked_by_role,
            )
            .as_str(),
        )
    }
    Ok(resp)
}

pub fn create_map_action_row(map_list: Vec<String>, vote_type: &VoteType) -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut menu = CreateSelectMenu::default();
    menu.custom_id("map_select");
    menu.placeholder(format!(
        "Select map to {}",
        vote_type.to_string().to_lowercase()
    ));
    let mut options = Vec::new();
    for map_name in map_list {
        options.push(create_menu_option(
            &map_name,
            &map_name.to_ascii_lowercase(),
        ))
    }
    menu.options(|f| f.set_options(options));
    ar.add_select_menu(menu);
    ar
}

pub fn create_server_action_row(
    server_list: Vec<ServerTemplates>,
    vote_type: &VoteType,
) -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut menu = CreateSelectMenu::default();
    menu.custom_id("server_select");
    menu.placeholder(format!(
        "Select server to {}",
        vote_type.to_string().to_lowercase()
    ));
    let mut options = Vec::new();
    for server in server_list {
        options.push(create_menu_option(&server.location, &server.location))
    }
    menu.options(|f| f.set_options(options));
    ar.add_select_menu(menu);
    ar
}

pub fn create_menu_option(label: &str, value: &str) -> CreateSelectMenuOption {
    let mut opt = CreateSelectMenuOption::default();
    // This is what will be shown to the user
    opt.label(label);
    // This is used to identify the selected value
    opt.value(value.to_ascii_lowercase());
    opt
}

pub fn create_sidepick_action_row() -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut menu = CreateSelectMenu::default();
    menu.custom_id("side_pick");
    menu.placeholder("Select starting side");
    menu.options(|f| {
        f.add_option(create_menu_option(&String::from("CT"), &String::from("ct")))
            .add_option(create_menu_option(&String::from("T"), &String::from("t")))
    });
    ar.add_select_menu(menu);
    ar
}

async fn no_team_resp(context: &Context<'_>, mci: &Arc<MessageComponentInteraction>) {
    mci.create_interaction_response(&context.discord(), |r| {
        r.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|d| {
                d.ephemeral(true)
                    .content("You are not part of either team currently setting up a match")
            })
    })
    .await
    .unwrap();
}

pub async fn start_server(
    context: &Context<'_>,
    pool: &PgPool,
    mci: &Arc<MessageComponentInteraction>,
    setup: &mut Setup,
    current_match: &mut MatchSeries,
) -> Result<ServerDuplicateResponse, Error> {
    println!("{:#?}", setup);
    mci.message.delete(&context.discord()).await?;
    let mut msg = mci.channel_id.send_message(&context.discord(), |m| {
        m.content("Match setup completed, starting server...\n[🌕🌑🌑🌑🌑]⏳ _Duplicating server template..._")
            .components(|c| c)
    }).await?;
    let dathost_config = DathostConfig {
        user: env::var("DATHOST_USER").unwrap(),
        password: env::var("DATHOST_PASSWORD").unwrap(),
    };
    let client = Client::new();
    let sync_url = format!(
        "https://dathost.net/api/0.1/game-servers/{server_id}/sync-files",
        server_id = encode(&setup.server_id.clone().unwrap())
    );
    client
        .post(sync_url)
        .basic_auth(&dathost_config.user, Some(&dathost_config.password))
        .send()
        .await?;
    let dupl_url = format!(
        "https://dathost.net/api/0.1/game-servers/{}/duplicate",
        encode(&setup.server_id.clone().unwrap())
    );
    let dupl_resp = client
        .post(dupl_url)
        .basic_auth(&dathost_config.user, Some(&dathost_config.password))
        .send()
        .await?
        .json::<ServerDuplicateResponse>()
        .await?;

    msg.edit(&context.discord(), |m| {
        m.content("Match setup completed, starting server...\n[🌕🌕🌑🌑]⏳ _Setting GSLT token..._")
    })
    .await?;

    let server_id = dupl_resp.id.clone();
    setup.server_hostname = dupl_resp.game.clone();
    setup.server_game_port = Some(dupl_resp.ports.game.clone());
    setup.server_gotv_port = Some(dupl_resp.ports.gotv.clone());
    let gslt = create_gslt(&server_id, setup.match_series.unwrap()).await?;
    println!("setting gslt '{}'", &gslt);
    client
        .put(format!(
            "https://dathost.net/api/0.1/game-servers/{}",
            encode(&server_id.to_string())
        ))
        .form(&[
            (
                "name",
                format!("match-server-{}", setup.match_series.unwrap()),
            ),
            ("csgo_settings.steam_game_server_login_token", gslt.clone()),
        ])
        .basic_auth(&dathost_config.user, Some(&dathost_config.password))
        .send()
        .await?;

    msg.edit(&context.discord(), |m| {
        m.content(
            "Match setup completed, starting server...\n[🌕🌕🌕🌑]⏳ _Start server from match config..._",
        )
    })
    .await?;

    setup.team_one_conn_str = Some(team_conn_str(setup.team_one.role, pool).await?);
    setup.team_two_conn_str = Some(team_conn_str(setup.team_two.role, pool).await?);
    println!(
        "starting match\nteam1 '{}'\nteam2: '{}'",
        setup.clone().team_one_conn_str.unwrap(),
        setup.clone().team_two_conn_str.unwrap()
    );
    let match_end_webhook_url = match env::var("MATCH_END_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => String::new(),
    };
    let round_end_webhook_url = match env::var("ROUND_END_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => String::new(),
    };
    let start_resp = match setup.series_type {
        Bo1 => {
            start_match(
                server_id,
                setup,
                client,
                &dathost_config,
                pool,
                match_end_webhook_url,
                round_end_webhook_url,
            )
            .await
        }
        Bo3 => {
            start_series_match(
                server_id,
                setup,
                client,
                &dathost_config,
                pool,
                match_end_webhook_url,
                round_end_webhook_url,
            )
            .await
        }
        Bo5 => {
            start_series_match(
                server_id,
                setup,
                client,
                &dathost_config,
                pool,
                match_end_webhook_url,
                round_end_webhook_url,
            )
            .await
        }
    };
    if let Err(err) = start_resp {
        eprintln!("{:#?}", err);
        return Err(Error::from(err));
    }
    let start_info = start_resp?.json::<DathostStartResponse>().await?;
    current_match
        .update_dathost_match(pool, start_info.id)
        .await?;
    Server::add(
        pool,
        setup.match_series.unwrap(),
        &dupl_resp.id,
        &dupl_resp.ip,
        dupl_resp.ports.game as i32,
        dupl_resp.ports.gotv as i32,
    )
    .await?;

    msg.edit(&context.discord(), |m| {
        m.content("Match setup completed, server started\n[🌕🌕🌕🌕]")
    })
    .await?;
    Ok(dupl_resp)
}

pub async fn create_gslt(server_id: &String, match_id: i32) -> Result<String> {
    let client = Client::new();
    let api_key = env::var("STEAM_API_KEY")?;
    let json = serde_json::to_string(&CreateGsltRequest {
        appid: 730,
        key: String::from(server_id),
        memo: match_id.to_string(),
    })?;
    let resp = client
        .post("https://api.steampowered.com/IGameServersService/CreateAccount/v1/")
        .query(&[("key", &&api_key), ("input_json", &&json)])
        .header("Content-Length", 0)
        .send()
        .await?
        .json::<SteamApiRootResponse>()
        .await?;
    Ok(resp.response.login_token)
}

pub async fn start_match(
    server_id: String,
    setup: &Setup,
    client: Client,
    dathost_config: &DathostConfig,
    pool: &PgPool,
    match_end_webhook_url: String,
    round_end_webhook_url: String,
) -> std::result::Result<Response, reqwest::Error> {
    let start_match_url = String::from("https://dathost.net/api/0.1/matches");
    let team_ct: String;
    let team_t: String;
    let team_ct_name: String;
    let team_t_name: String;
    let new_match = setup.maps_sel[0].clone();
    if setup.maps_sel[0].start_ct_team_role.unwrap() == setup.team_one.role {
        team_ct = setup.team_one_conn_str.clone().unwrap();
        team_ct_name = setup.team_one.name.clone();
        team_t = setup.team_two_conn_str.clone().unwrap();
        team_t_name = setup.team_two.name.clone();
    } else {
        team_ct = setup.team_two_conn_str.clone().unwrap();
        team_ct_name = setup.team_two.name.clone();
        team_t = setup.team_one_conn_str.clone().unwrap();
        team_t_name = setup.team_one.name.clone();
    }
    let map = Map::get(pool, new_match.map_id).await.unwrap();

    println!("starting match request...");
    client
        .post(&start_match_url)
        .form(&[
            ("game_server_id", &&server_id),
            ("map", &&map.name),
            ("team1_name", &&team_t_name),
            ("team2_name", &&team_ct_name),
            ("team1_steam_ids", &&team_t),
            ("team2_steam_ids", &&team_ct),
            ("enable_pause", &&String::from("true")),
            ("enable_tech_pause", &&String::from("true")),
            ("match_end_webhook_url", &&match_end_webhook_url),
            ("round_end_webhook_url", &&round_end_webhook_url),
        ])
        .basic_auth(&dathost_config.user, Some(&dathost_config.password))
        .send()
        .await
}

pub async fn start_series_match(
    server_id: String,
    setup: &mut Setup,
    client: Client,
    dathost_config: &DathostConfig,
    pool: &PgPool,
    match_end_webhook_url: String,
    round_end_webhook_url: String,
) -> std::result::Result<Response, reqwest::Error> {
    let start_match_url = String::from("https://dathost.net/api/0.1/match-series");
    let team_one = setup.team_one_conn_str.clone().unwrap();
    let team_one_name = setup.team_one.name.clone();
    let team_two = setup.team_two_conn_str.clone().unwrap();
    let team_two_name = setup.team_two.name.clone();
    let mut params: HashMap<&str, &str> = HashMap::new();
    let team_map = HashMap::from([
        (setup.team_one.role, "team1"),
        (setup.team_two.role, "team2"),
    ]);
    let maps = Map::get_all(pool, true).await.unwrap();
    let mut num_maps = "3";
    params.insert("game_server_id", server_id.as_str());
    params.insert("enable_pause", "true");
    params.insert("enable_tech_pause", "true");
    params.insert("match_end_webhook_url", &&match_end_webhook_url);
    params.insert("round_end_webhook_url", &&round_end_webhook_url);
    params.insert("team1_name", team_one_name.as_str());
    params.insert("team2_name", team_two_name.as_str());
    params.insert("team1_steam_ids", team_one.as_str());
    params.insert("team2_steam_ids", team_two.as_str());
    let map1 = maps
        .iter()
        .find(|m| m.id == setup.maps_sel[0].map_id)
        .unwrap();
    params.insert("map1", &map1.name);
    params.insert(
        "map1_start_ct",
        team_map
            .get(&setup.maps_sel[0].start_ct_team_role.unwrap())
            .unwrap(),
    );
    let map2 = maps
        .iter()
        .find(|m| m.id == setup.maps_sel[1].map_id)
        .unwrap();
    params.insert("map2", &map2.name);
    params.insert(
        "map2_start_ct",
        team_map
            .get(&setup.maps_sel[1].start_ct_team_role.unwrap())
            .unwrap(),
    );
    let map3 = maps
        .iter()
        .find(|m| m.id == setup.maps_sel[2].map_id)
        .unwrap();
    params.insert("map3", &map3.name);
    params.insert(
        "map3_start_ct",
        team_map
            .get(&setup.maps_sel[2].start_ct_team_role.unwrap())
            .unwrap(),
    );
    if setup.series_type == Bo5 {
        num_maps = "5";
        let map4 = maps
            .iter()
            .find(|m| m.id == setup.maps_sel[3].map_id)
            .unwrap();
        params.insert("map4", &map4.name);
        params.insert(
            "map4_start_ct",
            team_map
                .get(&setup.maps_sel[3].start_ct_team_role.unwrap())
                .unwrap(),
        );
        let map5 = maps
            .iter()
            .find(|m| m.id == setup.maps_sel[4].map_id)
            .unwrap();
        params.insert("map5", &map5.name);
        params.insert(
            "map5_start_ct",
            team_map
                .get(&setup.maps_sel[4].start_ct_team_role.unwrap())
                .unwrap(),
        );
    }
    params.insert("number_of_maps", num_maps);
    println!("{:#?}", params);
    client
        .post(&start_match_url)
        .form(&params)
        .basic_auth(&dathost_config.user, Some(&dathost_config.password))
        .send()
        .await
}

pub async fn team_conn_str(team: i64, pool: &PgPool) -> Result<String> {
    let steam_ids = SteamUser::get_by_team(pool, team).await?;
    let mut str: String = steam_ids
        .iter()
        .map(|u| {
            let mut steamid = SteamId::new(u.steam as u64).unwrap();
            steamid.set_universe(Universe::Public);
            steamid.steam2id()
        })
        .map(|s| format!("{},", s))
        .collect();
    str.remove(str.len() - 1);
    Ok(str)
}
