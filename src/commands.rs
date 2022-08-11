use std::borrow::Borrow;
use std::convert::TryFrom;
use std::str::FromStr;
use std::time::Duration;
use async_std::prelude::StreamExt;
use chrono::{Local};
use regex::Regex;
use urlencoding::encode;

use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::channel::Message;
use serenity::model::prelude::interaction::application_command::CommandDataOptionValue;
use serenity::model::prelude::Role;
use serenity::utils::MessageBuilder;
use match_bot::{create_match, create_user, delete_match, get_match, get_match_setup_steps, get_matches, get_next_team_match, update_match_schedule};
use match_bot::models::{Match, MatchState, NewMatch, SeriesType};

use crate::Setup;
use crate::SetupMap;
use crate::State::{Idle, MapVeto, SidePick};
use crate::StepType::{Pick, Veto};
use crate::utils::{admin_check, find_user_team_role, is_phase_allowed, user_team, eos_printout, get_maps, reset_setup, finish_setup, print_match_info, handle_bo3_setup, handle_bo5_setup, convert_steamid_to_64, get_pg_conn, handle_bo1_setup, print_veto_info, create_map_action_row, user_team_author, create_sidepick_action_row, create_server_conn_button_row};


pub(crate) async fn handle_help(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let mut commands = String::from("
`/setup` - start user's team's next match setup
`/schedule` - schedule match
`/matches` - list matches
`/maps` - list maps
`/ct` - pick CT side during side pick phase
`/t`- pick T side during side pick phase
`/pick` - pick map during map veto phase
`/ban` - ban map during map veto phase
`/help` - DMs you help text
");
    let admin_commands = String::from("
_These are privileged admin commands:_
`/addmatch` - add match to schedule
`/deletematch`- delete match from schedule
`/cancel` - cancel setup
    ");
    let admin_check = admin_check(context, msg).await;
    if let Ok(_result_str) = admin_check {
        commands.push_str(&admin_commands)
    }
    let response = MessageBuilder::new()
        .push(commands)
        .build();
    if let Ok(channel) = &msg.user.create_dm_channel(&context.http).await {
        if let Err(why) = channel.say(&context.http, &response).await {
            eprintln!("Error sending message: {:?}", why);
        }
    } else {
        eprintln!("Error sending .help dm");
    }
    String::from("Help info sent via DM")
}

pub(crate) async fn handle_setup(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let mut next_match = None;
    if let Ok(roles) = context.http.get_guild_roles(*msg.guild_id.unwrap().as_u64()).await {
        if let Ok(team_role) = find_user_team_role(roles, &msg.user, &context).await {
            let conn = get_pg_conn(&context).await;
            next_match = get_next_team_match(&conn, team_role.id.0 as i64);
        }
    } else {
        return String::from("You are not part of any team. Verify you have a role starting with `Team`");
    }
    if next_match.is_none() {
        return String::from("Your team does not have any scheduled matches");
    }
    let current_match = next_match.unwrap();
    let maps: Vec<String> = get_maps(&context).await;
    let mut data = context.data.write().await;
    let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
    setup.maps_remaining = maps;
    setup.series_type = current_match.series_type;
    setup.team_one = Some(current_match.team_one_role_id.clone());
    setup.team_two = Some(current_match.team_two_role_id.clone());
    setup.match_id = Some(current_match.id);
    setup.current_step = 0;
    setup.current_phase = MapVeto;
    let map_str: String = setup.maps_remaining.iter().map(|map| format!("- `{}`\n", map.to_lowercase())).collect();
    let mut result = match current_match.series_type {
        SeriesType::Bo1 => { handle_bo1_setup(setup.clone()).await }
        SeriesType::Bo3 => { handle_bo3_setup(setup.clone()).await }
        SeriesType::Bo5 => { handle_bo5_setup(setup.clone()).await }
    };
    setup.veto_pick_order = result.0;
    result.1.push_str("Remaining maps:\n");
    result.1.push_str(map_str.as_str());
    return result.1;
}

pub(crate) async fn handle_defense_option(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    if let Err(_err) = is_phase_allowed(context, msg, SidePick).await {
        return String::from("It is not the side pick phase");
    }
    let mut resp = String::new();
    if let Ok(user_role_id) = user_team(context, msg).await {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        if setup.maps[setup.current_step].picked_by == user_role_id as i64 {
            return String::from("It is not your turn to pick sides");
        }
        let picked_role_id = user_role_id;
        setup.maps[setup.current_step].start_defense_team_role_id = Some(user_role_id as i64);
        setup.maps[setup.current_step].start_attack_team_role_id = Option::from(if setup.clone().team_two.unwrap() == setup.maps[setup.current_step].start_defense_team_role_id.clone().unwrap() {
            setup.clone().team_one.unwrap() as i64
        } else {
            setup.clone().team_two.unwrap() as i64
        });
        if setup.maps.len() - 1 > setup.current_step {
            let next_pick = if setup.clone().team_two.unwrap() == setup.maps[setup.current_step + 1].picked_by {
                setup.clone().team_one.unwrap() as i64
            } else {
                setup.clone().team_two.unwrap() as i64
            };
            resp = format!("<@&{}> picked to start `defense` on `{}`. It is now <@&{}>'s turn to pick starting side on `{}`", &picked_role_id, setup.maps[setup.current_step].map.to_lowercase(), next_pick, setup.maps[setup.current_step + 1].map.to_lowercase());
            setup.current_step += 1;
            return resp;
        } else {
            resp = format!("<@&{}> picked to start `attack` on `{}`", &picked_role_id, setup.maps[setup.current_step].map.to_lowercase());
            resp.push_str(eos_printout(setup.clone()).as_str());
        };
    }
    finish_setup(context).await;
    if resp == String::new() {
        return String::from("There was an issue processing this option");
    }
    resp
}

pub async fn handle_buttons_test(context: &Context, msg: &Message) {
    let client = reqwest::Client::new();
    let url = "104.128.60.20:27015";
    let port_start = &url.find(':').unwrap_or(0_usize) + 1;
    let gotv_port = String::from(&url[port_start..url.len()]).parse::<i64>().unwrap_or(0) + 1;
    let gotv_url = format!("{}{}", &url[0..port_start], gotv_port);
    let url_link = format!("steam://connect/{}", &url);
    let gotv_link = format!("steam://connect/{}", &gotv_url);
    let resp = client
        .get(format!("https://tinyurl.com/api-create.php?url={}", encode(&url_link)))
        .send()
        .await
        .unwrap();
    let t_url = resp.text_with_charset("utf-8").await.unwrap();
    let resp = client
        .get(format!("https://tinyurl.com/api-create.php?url={}", encode(&gotv_link)))
        .send()
        .await
        .unwrap();
    let t_gotv_url = resp.text_with_charset("utf-8").await.unwrap();

    let m = msg.channel_id.send_message(&context, |m| m.content("Server started!")
        .components(|c|
            c.add_action_row(
                create_server_conn_button_row(&t_url, &t_gotv_url)
            ))
    ).await.unwrap();
    let mci =
        match m.await_component_interaction(&context).timeout(Duration::from_secs(3_600)).await {
            Some(ci) => ci,
            None => {
                m.reply(&context, "Timed out").await.unwrap();
                return;
            },
        };
    mci.create_interaction_response(&context, |r| {
        r.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(|d| {
                d.ephemeral(true).content(format!("Console: ||`connect {}`||\nGOTV: ||`connect {}`||", &url, &gotv_url))
        })
    })
        .await
        .unwrap();
}

pub(crate) async fn handle_new_setup(context: &Context, msg: &Message) {
    let mut next_match = None;
    if let Ok(roles) = context.http.get_guild_roles(*msg.guild_id.unwrap().as_u64()).await {
        if let Ok(team_role) = find_user_team_role(roles, &msg.author, &context).await {
            let conn = get_pg_conn(&context).await;
            next_match = get_next_team_match(&conn, team_role.id.0 as i64);
        }
    } else {
        msg.channel_id.send_message(&context, |m| m.content("You are not part of any team. Verify you have a role starting with `Team`")).await.unwrap();
        return;
    }
    if next_match.is_none() {
        msg.channel_id.send_message(&context, |m| m.content("Your team does not have any scheduled matches")).await.unwrap();
        return;
    }
    let current_match = next_match.unwrap();
    let maps: Vec<String> = get_maps(&context).await;
    let mut setup: Setup = Setup {
        maps_remaining: maps,
        maps: vec![],
        vetoes: vec![],
        series_type: current_match.series_type,
        team_one: Some(current_match.team_one_role_id.clone()),
        team_two: Some(current_match.team_two_role_id.clone()),
        match_id: Some(current_match.id),
        veto_pick_order: vec![],
        current_step: 0,
        current_phase: MapVeto,
    };
    let result = match current_match.series_type {
        SeriesType::Bo1 => { handle_bo1_setup(setup.clone()).await }
        SeriesType::Bo3 => { handle_bo3_setup(setup.clone()).await }
        SeriesType::Bo5 => { handle_bo5_setup(setup.clone()).await }
    };
    setup.veto_pick_order = result.0;
    let init_msg = result.1;
    let m = msg
        .channel_id
        .send_message(&context, |m| {
            m.content(init_msg).components(|c|
                c.add_action_row(create_map_action_row(setup.maps_remaining.clone())))
        })
        .await
        .unwrap();

    // Wait for the user to make a selection
    let mut cib = m.await_component_interactions(&context).build();

    while let Some(mci) = cib.next().await {
        match setup.current_phase {
            MapVeto => {
                let map_selected = mci.data.values.get(0).unwrap();
                let step_type = match setup.veto_pick_order[setup.current_step].step_type {
                    Veto => { String::from("banned") }
                    Pick => { String::from("picked") }
                };
                if let Ok(role_id) = user_team_author(&context, &setup, &mci).await {
                    if setup.veto_pick_order.get(setup.current_step).unwrap().team_role_id != role_id as i64 {
                        mci.create_interaction_response(&context, |r| {
                            r.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(
                                |d| {
                                    d.ephemeral(true).content("It is not your team's turn to pick or ban")
                                },
                            )
                        }).await.unwrap();
                        continue;
                    }

                    if setup.veto_pick_order[setup.current_step].step_type == Pick {
                        setup.maps.push(SetupMap {
                            map: map_selected.clone(),
                            picked_by: setup.veto_pick_order.get(setup.current_step).unwrap().team_role_id,
                            match_id: 0,
                            start_attack_team_role_id: None,
                            start_defense_team_role_id: None,
                        })
                    }
                    setup.veto_pick_order[setup.current_step].map = Some(String::from(map_selected));

                    if setup.veto_pick_order.len() == setup.current_step + 1 {
                        let first_map = setup.maps.get(0).unwrap();
                        mci.create_interaction_response(&context, |r| {
                            r.kind(InteractionResponseType::UpdateMessage).interaction_response_data(
                                |d| {
                                    d.content(format!("Map veto completed.\nIt is <@&{}> turn to pick starting side for `{}`", first_map.picked_by, first_map.map))
                                        .components(|c| c.add_action_row(create_sidepick_action_row()))
                                },
                            )
                        }).await.unwrap();
                        setup.current_step = 0;
                        setup.current_phase = SidePick;
                        continue;
                    }

                    let next_step_type = match setup.veto_pick_order[setup.current_step + 1].step_type {
                        Veto => { String::from("ban") }
                        Pick => { String::from("pick") }
                    };
                    let next_role_id = setup.veto_pick_order.get(setup.current_step + 1).unwrap().team_role_id;
                    let map_index = setup.maps_remaining.iter().position(|m| m == map_selected).unwrap();
                    setup.maps_remaining.remove(map_index);
                    mci.create_interaction_response(&context, |r| {
                        r.kind(InteractionResponseType::UpdateMessage).interaction_response_data(
                            |d| {
                                d.content(format!("<@&{}> {} `{}`\nIt is <@&{}> turn to {}", role_id, step_type, map_selected, next_role_id, next_step_type))
                                    .components(|c| c.add_action_row(create_map_action_row(setup.maps_remaining.clone())))
                            },
                        )
                    }).await.unwrap();
                    setup.current_step = setup.current_step + 1;
                } else {
                    mci.create_interaction_response(&context, |r| {
                        r.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(
                            |d| {
                                d.ephemeral(true).content("You are not part of either team currently setting up a match")
                            },
                        )
                    }).await.unwrap();
                }
            }
            SidePick => {
                let option_selected = mci.data.values.get(0).unwrap();
                if let Ok(role_id) = user_team_author(&context, &setup, &mci).await {
                    let other_role_id = if setup.team_one.unwrap() == role_id as i64 { setup.team_two.unwrap() } else { setup.team_one.unwrap() };
                    if other_role_id == role_id as i64 {
                        mci.create_interaction_response(&context, |r| {
                            r.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(
                                |d| {
                                    d.ephemeral(true).content("It is not your team's turn to pick sides")
                                },
                            )
                        }).await.unwrap();
                        continue;
                    }
                    if setup.maps.len() > setup.current_step + 1 {
                        let current_map = &setup.maps.get(setup.current_step).unwrap().map;
                        let next_map = &setup.maps.get(setup.current_step + 1).unwrap().map;
                        mci.create_interaction_response(&context, |r| {
                            r.kind(InteractionResponseType::UpdateMessage).interaction_response_data(
                                |d| {
                                    d.content(format!("<@&{}> picked to start {} on `{}`\nIt is <@&{}> turn to pick starting side on {}", role_id, option_selected.to_uppercase(), current_map, other_role_id, next_map))
                                        .components(|c| c.add_action_row(create_sidepick_action_row()))
                                },
                            )
                        }).await.unwrap();
                    }
                    if option_selected == &String::from("CT") {
                        setup.maps[setup.current_step].start_defense_team_role_id = Some(other_role_id);
                        setup.maps[setup.current_step].start_attack_team_role_id = Some(role_id as i64);
                    } else {
                        setup.maps[setup.current_step].start_attack_team_role_id = Some(other_role_id);
                        setup.maps[setup.current_step].start_defense_team_role_id = Some(role_id as i64);
                    }
                    setup.current_step = setup.current_step + 1;
                    if setup.maps.len() == setup.current_step {
                        msg.channel_id.send_message(&context, |m| m.content("Match setup completed, starting server...")).await.unwrap();
                        m.delete(&context).await.expect("Expected message to delete");
                        handle_server_start(&context, msg, &setup);
                    }
                } else {
                    mci.create_interaction_response(&context, |r| {
                        r.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(
                            |d| {
                                d.ephemeral(true).content("You are not part of either team currently setting up a match")
                            },
                        )
                    }).await.unwrap();
                }
            }
            _ => {}
        }
    }
}

fn handle_server_start(context: &Context, msg: &Message, setup: &Setup) {
    println!("{:#?}", setup);
}

pub(crate) async fn handle_attack_option(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    if let Err(_err) = is_phase_allowed(context, msg, SidePick).await {
        return String::from("It is not the side pick phase");
    }
    let mut resp = String::new();
    if let Ok(user_role_id) = user_team(context, msg).await {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        if setup.maps[setup.current_step].picked_by == user_role_id as i64 {
            return String::from("It is not your turn to pick sides");
        }
        let picked_role_id = user_role_id;
        setup.maps[setup.current_step].start_attack_team_role_id = Some(user_role_id as i64);
        setup.maps[setup.current_step].start_defense_team_role_id = Option::from(if setup.clone().team_two.unwrap() == setup.maps[setup.current_step].start_attack_team_role_id.clone().unwrap() {
            setup.clone().team_one.unwrap() as i64
        } else {
            setup.clone().team_two.unwrap() as i64
        });
        if setup.maps.len() - 1 > setup.current_step {
            let next_pick = if setup.clone().team_two.unwrap() == setup.maps[setup.current_step + 1].picked_by {
                setup.clone().team_one
            } else {
                setup.clone().team_two
            };
            resp = format!("<@&{}> picked to start `attack` on `{}`. It is now <@&{}>'s turn to pick starting side on `{}`", &picked_role_id, setup.maps[setup.current_step].map.to_lowercase(), next_pick.unwrap(), setup.maps[setup.current_step + 1].map.to_lowercase());
            setup.current_step += 1;
            return resp;
        } else {
            resp = format!("<@&{}> picked to start `attack` on `{}`", &picked_role_id, setup.maps[setup.current_step].map.to_lowercase());
            resp.push_str(eos_printout(setup.clone()).as_str());
        };
    }
    finish_setup(context).await;
    if resp == String::new() {
        return String::from("There was an issue processing this option");
    }
    resp
}


pub(crate) async fn handle_pick_option(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    if let Err(err) = is_phase_allowed(context, msg, MapVeto).await {
        return err;
    }
    {
        let data = context.data.write().await;
        let setup: &Setup = data.get::<Setup>().unwrap();
        if setup.veto_pick_order.get(setup.current_step).unwrap().step_type != Pick {
            return String::from("It is not your turn to pick");
        }
    }
    if let Ok(user_role_id) = user_team(context, msg).await {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        if setup.veto_pick_order.get(setup.current_step).unwrap().team_role_id != user_role_id as i64 {
            return String::from("It is not your turn to pick");
        }
        let option = msg.data
            .options
            .get(0)
            .expect("Expected map name option")
            .resolved
            .as_ref()
            .expect("Expected object");
        if let CommandDataOptionValue::String(map) = option {
            if !setup.maps_remaining.contains(&map) {
                return String::from("Select a remaining map");
            }
            setup.veto_pick_order[setup.current_step].map = Some(String::from(map));
            let map_index = setup.maps_remaining.iter().position(|m| m == map).unwrap();
            setup.maps_remaining.remove(map_index);
            let picked_by_team = setup.veto_pick_order[setup.current_step].team_role_id.clone();
            setup.maps.push(SetupMap {
                match_id: setup.match_id.unwrap(),
                map: map.clone(),
                picked_by: picked_by_team.clone(),
                start_attack_team_role_id: None,
                start_defense_team_role_id: None,
            });
            let mut resp = format!("<@&{}> picked `{}`. Maps remaining:\n", &picked_by_team, map.to_lowercase());
            let map_str: String = setup.maps_remaining.iter().map(|map| format!("- `{}`\n", map.to_lowercase())).collect();
            resp.push_str(map_str.as_str());
            setup.current_step += 1;
            if setup.current_step >= setup.veto_pick_order.len() {
                setup.current_phase = SidePick;
                resp = format!("<@&{}> picked `{}`. Map veto has concluded.\n\nTeams will now pick starting sides.\n", &picked_by_team, map.to_lowercase());
                setup.current_step = 0;
                resp.push_str(format!("It is <@&{}>'s turn to pick starting side for `{}`\nUse `/t` or `/ct` to select starting side", setup.clone().team_two.unwrap(), setup.maps[0].map.to_lowercase()).as_str());
                return resp;
            }
            resp.push_str(format!("It is <@&{}>'s turn to `{}`", setup.veto_pick_order[setup.current_step].team_role_id, setup.veto_pick_order[setup.current_step].step_type.to_string()).as_str());
            return resp;
        }
    }
    String::from("There was an issue picking a map")
}

pub(crate) async fn handle_ban_option(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    if let Err(err) = is_phase_allowed(context, msg, MapVeto).await {
        return err;
    }
    {
        let data = context.data.write().await;
        let setup: &Setup = data.get::<Setup>().unwrap();
        if setup.veto_pick_order.get(setup.current_step).unwrap().step_type != Veto {
            return String::from("It is not your turn to ban");
        }
    }
    if let Ok(user_role_id) = user_team(context, msg).await {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        if setup.veto_pick_order.get(setup.current_step).unwrap().team_role_id != user_role_id as i64 {
            return String::from("It is not your turn to ban");
        }
        let option = msg.data
            .options
            .get(0)
            .expect("Expected map name option")
            .resolved
            .as_ref()
            .expect("Expected object");
        if let CommandDataOptionValue::String(map) = option {
            if !setup.maps_remaining.contains(&map) {
                return String::from("Select a remaining map");
            }
            setup.veto_pick_order[setup.current_step].map = Some(String::from(map));
            let map_index = setup.maps_remaining.iter().position(|m| m == map).unwrap();
            setup.maps_remaining.remove(map_index);
            let banned_by_team = setup.veto_pick_order[setup.current_step].team_role_id.clone();
            let mut resp = format!("<@&{}> banned `{}`. Maps remaining:\n", &banned_by_team, map);
            let map_str: String = setup.maps_remaining.iter().map(|map| format!("- `{}`\n", map.to_lowercase())).collect();
            resp.push_str(map_str.as_str());
            setup.current_step += 1;
            if setup.current_step >= setup.veto_pick_order.len() {
                setup.current_phase = SidePick;
                setup.current_step = 0;
                resp = String::from("Map veto has concluded. Teams will now pick starting sides. \n");
                resp.push_str(format!("It is <@&{}>'s turn to pick starting side for `{}`\nUse `/t` or `/ct` to select starting side", setup.clone().team_two.unwrap(), setup.maps[0].map.to_lowercase()).as_str());
                return resp;
            }
            resp.push_str(format!("It is <@&{}>'s turn to `{}`", setup.veto_pick_order[setup.current_step].team_role_id, setup.veto_pick_order[setup.current_step].step_type.to_string()).as_str());
            return resp;
        }
    }
    String::from("There was an issue banning a map")
}

pub(crate) async fn handle_map_list(context: &Context) -> String {
    let maps: Vec<String> = get_maps(&context).await;
    let map_str: String = maps.iter().map(|map| format!("- `{}`\n", map)).collect();
    return MessageBuilder::new()
        .push_line("Current map pool:")
        .push(map_str)
        .build();
}

pub(crate) async fn handle_schedule(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let option_one = msg.data
        .options
        .get(0)
        .expect("Expected date option")
        .resolved
        .as_ref()
        .expect("Expected object");
    let mut date: Option<String> = None;
    if let CommandDataOptionValue::String(date_str) = option_one {
        date = Some(date_str.clone());
    }
    if let Ok(roles) = context.http.get_guild_roles(*msg.guild_id.unwrap().as_u64()).await {
        let team_roles: Vec<Role> = roles.into_iter().filter(|r| r.name.starts_with("Team")).collect();
        let mut user_team_role: Option<Role> = None;
        for team_role in team_roles {
            if let Ok(has_role) = msg.user.has_role(&context.http, team_role.guild_id, team_role.id).await {
                if !has_role { continue; }
                user_team_role = Some(team_role);
                break;
            }
        }
        if let Some(team_role) = user_team_role {
            let conn = get_pg_conn(context).await;
            return if let Some(next_match) = get_next_team_match(&conn, team_role.id.0 as i64) {
                update_match_schedule(&conn, next_match.id, date.clone().unwrap());
                let resp_str = format!("Your next match (<@&{}> vs <@&{}>) is scheduled for `{}`", next_match.team_one_role_id, next_match.team_two_role_id, &date.unwrap());
                resp_str
            } else {
                String::from("Your team does not have any scheduled matches")
            };
        }
    }
    String::from("You are not part of any team. Verify you have a role starting with `Team`")
}

pub(crate) async fn handle_match(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let option_one = msg.data
        .options
        .get(0)
        .expect("Expected match id")
        .resolved
        .as_ref()
        .expect("Expected object");

    return if let CommandDataOptionValue::String(match_id) = option_one {
        let match_id_parsed = match_id.clone().parse::<i32>().unwrap();
        let conn = get_pg_conn(context).await;
        let m: Match = get_match(&conn, match_id_parsed);
        let steps = get_match_setup_steps(&conn, match_id_parsed);
        let mut row = String::new();
        row.push_str(print_match_info(&m, false).as_str());
        row.push_str(print_veto_info(steps, &m).as_str());
        row
    } else {
        String::from("Discord API error")
    };
}

pub(crate) async fn handle_matches(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let option_one = msg.data
        .options
        .get(0);
    let option_two = msg.data
        .options
        .get(1);
    let mut show_completed = false;
    if let Some(option) = option_two {
        if let Some(CommandDataOptionValue::Boolean(display)) = &option.resolved {
            show_completed = *display;
        }
    }
    let mut show_ids = false;
    if let Some(option) = option_one {
        if let Some(CommandDataOptionValue::Boolean(display)) = &option.resolved {
            show_ids = *display;
        }
    }

    let conn = get_pg_conn(&context).await;
    let matches = get_matches(&conn, 20, show_completed);
    if matches.is_empty() {
        return String::from("No matches have been added");
    }
    let matches_str: String = matches.iter()
        .map(|m| {
            let mut row = String::new();
            row.push_str(print_match_info(m, show_ids).as_str());
            row
        })
        .collect();
    matches_str
}

pub(crate) async fn handle_add_match(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let admin_check = admin_check(context, msg).await;
    if let Err(error) = admin_check { return error; }
    let option_one = msg.data
        .options
        .get(0)
        .expect("Expected teamone option")
        .resolved
        .as_ref()
        .expect("Expected object");
    let option_two = msg.data
        .options
        .get(1)
        .expect("Expected teamtwo option")
        .resolved
        .as_ref()
        .expect("Expected object");
    let option_three = msg.data
        .options
        .get(2)
        .expect("Expected series type option")
        .resolved
        .as_ref()
        .expect("Expected object");
    let option_four = msg.data
        .options
        .get(3);
    let mut team_one_role_id = 0;
    let mut team_one_name = "";
    let mut team_two_role_id = 0;
    let mut team_two_name = "";
    let mut series_type = SeriesType::Bo1;
    if let CommandDataOptionValue::Role(team_one_role) = option_one {
        team_one_role_id = team_one_role.id.0;
        team_one_name = &team_one_role.name.as_str().clone();
    }
    if let CommandDataOptionValue::Role(team_two_role) = option_two {
        team_two_role_id = team_two_role.id.0;
        team_two_name = &team_two_role.name.as_str().clone();
    }

    if let CommandDataOptionValue::String(s_type) = option_three {
        series_type = SeriesType::from_str(s_type).unwrap().clone();
    }

    let mut note = String::new();
    if let Some(option) = option_four {
        if let Some(CommandDataOptionValue::String(option_value)) = &option.resolved {
            note = option_value.clone();
        }
    }

    let mut note_content = None;
    if note != String::new() {
        note_content = Some(note.as_str());
    }
    let new_match = NewMatch {
        team_one_role_id: &(team_one_role_id as i64),
        team_one_name,
        team_two_role_id: &(team_two_role_id as i64),
        team_two_name,
        note: note_content,
        series_type: &series_type,
        date_added: &Local::now().naive_local(),
        match_state: &MatchState::Entered,
    };
    let conn = get_pg_conn(context).await;
    create_match(&conn, new_match);
    String::from("Successfully added new match")
}

pub(crate) async fn handle_delete_match(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let admin_check = admin_check(context, msg).await;
    if let Err(error) = admin_check { return error; }
    let option_one = msg.data
        .options
        .get(0)
        .expect("Expected matchid option")
        .resolved
        .as_ref()
        .expect("Expected object");
    let mut parsed_match_id: Option<i32> = None;
    if let CommandDataOptionValue::Integer(match_id) = option_one {
        if let Ok(id) = i32::try_from(match_id.clone()) {
            parsed_match_id = Some(id);
        }
    }
    if let Some(id) = &parsed_match_id {
        let conn = get_pg_conn(context).await;
        delete_match(&conn, id.clone());
    } else {
        return String::from("Cannot parse match id input");
    }
    String::from("Successfully deleted match")
}

pub(crate) async fn handle_steam_id(context: &Context, inc_command: &ApplicationCommandInteraction) -> String {
    let conn = get_pg_conn(&context).await;
    let option = inc_command.data
        .options
        .get(0)
        .expect("Expected steamid option")
        .resolved
        .as_ref()
        .expect("Expected object");
    if let CommandDataOptionValue::String(steamid) = option {
        let steam_id_regex = Regex::new("^STEAM_[0-5]:[01]:\\d+$").unwrap();
        if !steam_id_regex.is_match(&steamid) {
            return String::from(" invalid Steam ID input format. Please follow this example: `STEAM_0:1:12345678`");
        }
        let steamid_64 = convert_steamid_to_64(&steamid);
        create_user(conn.borrow(), &(inc_command.user.id.0 as i64), steamid.clone().as_str());
        let response = MessageBuilder::new()
            .push("Updated steamid for ")
            .mention(&inc_command.user)
            .push(" to `")
            .push(&steamid)
            .push("`\n")
            .push_line("Your steam community profile (please double check this is correct):")
            .push_line(format!("https://steamcommunity.com/profiles/{}", steamid_64))
            .build();
        return String::from(response);
    }
    return String::from("Discord API error");
}

pub(crate) async fn handle_cancel(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let admin_check = admin_check(context, msg).await;
    if let Err(error) = admin_check { return error; }
    let maps = get_maps(context).await;
    let mut data = context.data.write().await;
    let draft: &mut Setup = data.get_mut::<Setup>().unwrap();
    if draft.current_phase == Idle {
        return String::from(" command only valid during `/setup` process");
    }
    reset_setup(draft, maps);
    String::from("`/setup` process cancelled.")
}

