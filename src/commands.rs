use std::str::FromStr;
use chrono::{NaiveDate, Utc};


use serenity::client::Context;
use serenity::model::interactions::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::application_command::ApplicationCommandInteractionDataOptionValue;
use serenity::model::prelude::Role;
use serenity::utils::MessageBuilder;
use uuid::Uuid;

use crate::{Setup, Maps, Match, Matches, MatchState, RolePartial, ScheduleInfo, SeriesType, SetupStep, SeriesMap};
use crate::MatchState::Completed;
use crate::State::{Idle, MapVeto, SidePick};
use crate::StepType::{Pick, Veto};
use crate::utils::{admin_check, write_to_file, find_user_team_role, is_phase_allowed, user_team, eos_printout, get_maps, reset_setup, finish_setup, print_veto_info, print_match_info};


pub(crate) async fn handle_help(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let mut commands = String::from("
`/setup` - start user's team's next match setup
`/schedule` - schedule match
`/matches` - list matches
`/maps` - list maps
`/defense` - pick defense side during side pick phase
`/attack`- pick attack side during side pick phase
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
    let option = msg.data
        .options
        .get(0)
        .expect("Expected match type option")
        .resolved
        .as_ref()
        .expect("Expected object");

    let mut next_match = None;
    if let Ok(roles) = context.http.get_guild_roles(*msg.guild_id.unwrap().as_u64()).await {
        if let Ok(team_role) = find_user_team_role(roles, &msg.user, &context).await {
            let mut data = context.data.write().await;
            let matches: &mut Vec<Match> = data.get_mut::<Matches>().unwrap();
            for m in matches.iter_mut() {
                if m.match_state == Completed { continue; }
                if m.team_one.id != team_role.id && m.team_two.id != team_role.id { continue; }
                next_match = Some(m.clone());
                break;
            }
        }
    } else {
        return String::from("You are not part of any team. Verify you have a role starting with `Team`");
    }
    if next_match.is_none() {
        return String::from("Your team does not have any scheduled matches");
    }
    let current_match = next_match.unwrap();
    let mut data = context.data.write().await;
    let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
    if let ApplicationCommandInteractionDataOptionValue::String(match_type) = option {
        setup.series_type = SeriesType::from_str(match_type).unwrap();
    }
    setup.team_one = Some(current_match.team_one.clone());
    setup.team_two = Some(current_match.team_two.clone());
    setup.match_id = Some(current_match.id);
    setup.current_step = 0;
    setup.current_phase = MapVeto;
    let map_str: String = setup.maps_remaining.iter().map(|map| format!("- `{}`\n", map.to_uppercase())).collect();
    if let ApplicationCommandInteractionDataOptionValue::String(match_type) = option {
        let mut result = match SeriesType::from_str(match_type).unwrap() {
            SeriesType::Bo1 => { handle_bo1_setup(msg, setup.clone()).await }
            SeriesType::Bo3 => { handle_bo3_setup(msg, setup.clone()).await }
            SeriesType::Bo5 => { handle_bo5_setup(msg, setup.clone()).await }
        };
        setup.veto_pick_order = result.0;
        result.1.push_str("Remaining maps:\n");
        result.1.push_str(map_str.as_str());
        return result.1;
    }
    String::from("Setup encountered an error")
}

pub(crate) async fn handle_bo1_setup(_msg: &ApplicationCommandInteraction, setup: Setup) -> (Vec<SetupStep>, String) {
    return (vec![
        SetupStep { step_type: Veto, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Veto, team: setup.clone().team_one.unwrap(), map: None },
        SetupStep { step_type: Veto, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Veto, team: setup.clone().team_one.unwrap(), map: None },
        SetupStep { step_type: Veto, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_one.unwrap(), map: None },
    ], format!("Best of 1 option selected. Starting map veto. <@&{}> bans first.\n", &setup.team_one.unwrap().id));
}

pub(crate) async fn handle_bo3_setup(_msg: &ApplicationCommandInteraction, setup: Setup) -> (Vec<SetupStep>, String) {
    return (vec![
        SetupStep { step_type: Veto, team: setup.clone().team_one.unwrap(), map: None },
        SetupStep { step_type: Veto, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_one.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Veto, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_one.unwrap(), map: None },
    ], format!("Best of 3 option selected. Starting map veto. <@&{}> bans first.\n", &setup.team_one.unwrap().id));
}

pub(crate) async fn handle_bo5_setup(_msg: &ApplicationCommandInteraction, setup: Setup) -> (Vec<SetupStep>, String) {
    return (vec![
        SetupStep { step_type: Veto, team: setup.clone().team_one.unwrap(), map: None },
        SetupStep { step_type: Veto, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_one.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_one.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_two.unwrap(), map: None },
        SetupStep { step_type: Pick, team: setup.clone().team_one.unwrap(), map: None },
    ], format!("Best of 5 option selected. Starting map veto. <@&{}> bans first.\n", &setup.team_one.unwrap().id));
}

pub(crate) async fn handle_defense_option(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    if let Err(_err) = is_phase_allowed(context, msg, SidePick).await {
        return String::from("It is not the side pick phase");
    }
    let mut resp = String::new();
    if let Ok(user_role_partial) = user_team(context, msg).await {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        if setup.maps[setup.current_step].picked_by == user_role_partial {
            return String::from("It is not your turn to pick sides");
        }
        let picked_role_id = user_role_partial.id;
        setup.maps[setup.current_step].start_defense = Some(user_role_partial);
        setup.maps[setup.current_step].start_attack = if setup.clone().team_two.unwrap().id == setup.maps[setup.current_step].start_defense.clone().unwrap().id {
            setup.clone().team_one
        } else {
            setup.clone().team_two
        };
        if setup.maps.len() - 1 > setup.current_step {
            let next_pick = if setup.clone().team_two.unwrap().id == setup.maps[setup.current_step + 1].picked_by.id {
                setup.clone().team_one
            } else {
                setup.clone().team_two
            };
            resp = format!("<@&{}> picked to start `defense` on `{}`. It is now <@&{}>'s turn to pick starting side on `{}`", &picked_role_id, setup.maps[setup.current_step].map.to_uppercase(), next_pick.unwrap().id, setup.maps[setup.current_step + 1].map.to_uppercase());
            setup.current_step += 1;
            return resp;
        } else {
            resp = format!("<@&{}> picked to start `attack` on `{}`", &picked_role_id, setup.maps[setup.current_step].map.to_uppercase());
            resp.push_str(eos_printout(setup.clone()).as_str());
        };
    }
    finish_setup(context).await;
    if resp == String::new() {
        return String::from("There was an issue processing this option");
    }
    resp
}

pub(crate) async fn handle_attack_option(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    if let Err(_err) = is_phase_allowed(context, msg, SidePick).await {
        return String::from("It is not the side pick phase");
    }
    let mut resp = String::new();
    if let Ok(user_role_partial) = user_team(context, msg).await {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        if setup.maps[setup.current_step].picked_by == user_role_partial {
            return String::from("It is not your turn to pick sides");
        }
        let picked_role_id = user_role_partial.id;
        setup.maps[setup.current_step].start_attack = Some(user_role_partial);
        setup.maps[setup.current_step].start_defense = if setup.clone().team_two.unwrap().id == setup.maps[setup.current_step].start_attack.clone().unwrap().id {
            setup.clone().team_one
        } else {
            setup.clone().team_two
        };
        if setup.maps.len() - 1 > setup.current_step {
            let next_pick = if setup.clone().team_two.unwrap().id == setup.maps[setup.current_step + 1].picked_by.id {
                setup.clone().team_one
            } else {
                setup.clone().team_two
            };
            resp = format!("<@&{}> picked to start `attack` on `{}`. It is now <@&{}>'s turn to pick starting side on `{}`", &picked_role_id, setup.maps[setup.current_step].map.to_uppercase(), next_pick.unwrap().id, setup.maps[setup.current_step + 1].map.to_uppercase());
            setup.current_step += 1;
            return resp;
        } else {
            resp = format!("<@&{}> picked to start `attack` on `{}`", &picked_role_id, setup.maps[setup.current_step].map.to_uppercase());
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
    if let Ok(user_role_partial) = user_team(context, msg).await {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        if setup.veto_pick_order.get(setup.current_step).unwrap().team.id != user_role_partial.id {
            return String::from("It is not your turn to pick");
        }
        let option = msg.data
            .options
            .get(0)
            .expect("Expected map name option")
            .resolved
            .as_ref()
            .expect("Expected object");
        if let ApplicationCommandInteractionDataOptionValue::String(map) = option {
            if !setup.maps_remaining.contains(map) {
                return String::from("Select a remaining map");
            }
            setup.veto_pick_order[setup.current_step].map = Some(String::from(map));
            let map_index = setup.maps_remaining.iter().position(|m| m == map).unwrap();
            setup.maps_remaining.remove(map_index);
            let picked_by_team = setup.veto_pick_order[setup.current_step].team.clone();
            setup.maps.push(SeriesMap {
                map: map.clone(),
                picked_by: picked_by_team.clone(),
                start_attack: None,
                start_defense: None,
            });
            let mut resp = format!("<@&{}> picked `{}`. Maps remaining:\n", &picked_by_team.id, map.to_uppercase());
            let map_str: String = setup.maps_remaining.iter().map(|map| format!("- `{}`\n", map.to_uppercase())).collect();
            resp.push_str(map_str.as_str());
            setup.current_step += 1;
            if setup.current_step >= setup.veto_pick_order.len() {
                setup.current_phase = SidePick;
                resp = format!("<@&{}> picked `{}`. Map veto has concluded.\n\nTeams will now pick starting sides.\n", &picked_by_team.id, map.to_uppercase());
                setup.current_step = 0;
                resp.push_str(format!("It is <@&{}>'s turn to pick starting side for `{}`\nUse `/attack` or `/defense` to select starting side", setup.clone().team_two.unwrap().id, setup.maps[0].map.to_uppercase()).as_str());
                return resp;
            }
            resp.push_str(format!("It is <@&{}>'s turn to `{}`", setup.veto_pick_order[setup.current_step].team.id, setup.veto_pick_order[setup.current_step].step_type.to_string()).as_str());
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
    if let Ok(user_role_partial) = user_team(context, msg).await {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        if setup.veto_pick_order.get(setup.current_step).unwrap().team.id != user_role_partial.id {
            return String::from("It is not your turn to ban");
        }
        let option = msg.data
            .options
            .get(0)
            .expect("Expected map name option")
            .resolved
            .as_ref()
            .expect("Expected object");
        if let ApplicationCommandInteractionDataOptionValue::String(map) = option {
            if !setup.maps_remaining.contains(map) {
                return String::from("Select a remaining map");
            }
            setup.veto_pick_order[setup.current_step].map = Some(String::from(map));
            let map_index = setup.maps_remaining.iter().position(|m| m == map).unwrap();
            setup.maps_remaining.remove(map_index);
            let banned_by_team = setup.veto_pick_order[setup.current_step].team.clone();
            let mut resp = format!("<@&{}> banned `{}`. Maps remaining:\n", &banned_by_team.id, map);
            let map_str: String = setup.maps_remaining.iter().map(|map| format!("- `{}`\n", map.to_uppercase())).collect();
            resp.push_str(map_str.as_str());
            setup.current_step += 1;
            if setup.current_step >= setup.veto_pick_order.len() {
                setup.current_phase = SidePick;
                setup.current_step = 0;
                resp = String::from("Map veto has concluded. Teams will now pick starting sides. \n");
                resp.push_str(format!("It is <@&{}>'s turn to pick starting side for `{}`\nUse `/attack` or `/defense` to select starting side", setup.clone().team_two.unwrap().id, setup.maps[0].map.to_uppercase()).as_str());
                return resp;
            }
            resp.push_str(format!("It is <@&{}>'s turn to `{}`", setup.veto_pick_order[setup.current_step].team.id, setup.veto_pick_order[setup.current_step].step_type.to_string()).as_str());
            return resp;
        }
    }
    String::from("There was an issue banning a map")
}

pub(crate) async fn handle_map_list(context: &Context) -> String {
    let data = context.data.write().await;
    let maps: &Vec<String> = data.get::<Maps>().unwrap();
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
    let option_two = msg.data
        .options
        .get(1)
        .expect("Expected time option")
        .resolved
        .as_ref()
        .expect("Expected object");
    let mut date: Option<NaiveDate> = None;
    let mut match_date_str = String::from("");
    let mut time: Option<String> = None;
    if let ApplicationCommandInteractionDataOptionValue::String(date_str) = option_one {
        match_date_str = String::from(date_str);
        if let Ok(date_result) = NaiveDate::parse_from_str(date_str, "%m/%d/%Y") {
            date = Some(date_result);
        } else {
            return String::from("Incorrect date format. Please use correct format (Month/Day/Year) i.e. `12/23/2022`");
        }
    }
    if let ApplicationCommandInteractionDataOptionValue::String(time_str) = option_two {
        time = Some(time_str.to_string());
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
            let mut data = context.data.write().await;
            let matches: &mut Vec<Match> = data.get_mut::<Matches>().unwrap();
            let mut resp_str = String::new();
            for m in matches.iter_mut() {
                if m.team_one.id != team_role.id && m.team_two.id != team_role.id { continue; }
                m.schedule_info = Some(ScheduleInfo { date: date.unwrap(), time_str: time.clone().unwrap() });
                resp_str = format!("Your next match (<@&{}> vs <@&{}>) is scheduled for `{} @ {}`", m.team_one.id.as_u64(), m.team_two.id.as_u64(), &match_date_str, time.clone().unwrap());
            }
            write_to_file("matches.json", serde_json::to_string(matches).unwrap()).await;
            if !resp_str.is_empty() {
                return resp_str;
            }
            return String::from("Your team does not have any scheduled matches");
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

    if let ApplicationCommandInteractionDataOptionValue::String(match_id) = option_one {
        let data = context.data.write().await;
        let matches: &Vec<Match> = data.get::<Matches>().unwrap();
        if matches.is_empty() {
            return String::from("No matches have been added");
        }
        let id = Uuid::from_str(match_id);
        if let Ok(uuid) = id {
            let find_match = matches.iter().find(|m| m.id == uuid);
            if let Some(m) = find_match {
                let mut row = String::new();
                row.push_str(print_match_info(m, false).as_str());
                row.push_str(print_veto_info(m).as_str());
                return row;
            }
        } else {
            return String::from("Invalid match id format");
        }
    }
    String::from("Discord API error")
}

pub(crate) async fn handle_matches(context: &Context, msg: &ApplicationCommandInteraction) -> String {
    let option_one = msg.data
        .options
        .get(0);
    let option_two = msg.data
        .options
        .get(1);
    let data = context.data.write().await;
    let matches: &Vec<Match> = data.get::<Matches>().unwrap();
    if matches.is_empty() {
        return String::from("No matches have been added");
    }
    let mut show_completed = false;
    if let Some(option) = option_two {
        if let Some(ApplicationCommandInteractionDataOptionValue::Boolean(display)) = &option.resolved {
            show_completed = *display;
        }
    }
    let matches_str: String = matches.iter()
        .filter(|m| if show_completed {
            m.match_state == Completed
        } else {
            m.match_state != Completed
        })
        .map(|m| {
            let mut row = String::new();
            let mut show_ids = false;
            if let Some(option) = option_one {
                if let Some(ApplicationCommandInteractionDataOptionValue::Boolean(display)) = &option.resolved {
                    show_ids = *display;
                }
            }
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
        .get(2);
    let mut team_one = None;
    let mut team_two = None;
    if let ApplicationCommandInteractionDataOptionValue::Role(team_one_role) = option_one {
        team_one = Some(RolePartial { id: team_one_role.id, name: team_one_role.name.to_string(), guild_id: team_one_role.guild_id });
    }
    if let ApplicationCommandInteractionDataOptionValue::Role(team_two_role) = option_two {
        team_two = Some(RolePartial { id: team_two_role.id, name: team_two_role.name.to_string(), guild_id: team_two_role.guild_id });
    }
    let mut new_match = Match {
        id: Uuid::new_v4(),
        team_one: team_one.unwrap(),
        team_two: team_two.unwrap(),
        note: None,
        date_added: Utc::now(),
        match_state: MatchState::Entered,
        schedule_info: None,
        setup_info: None,
    };
    if let Some(option) = option_three {
        if let Some(ApplicationCommandInteractionDataOptionValue::String(option_value)) = &option.resolved {
            new_match.note = Option::from(option_value.clone());
        }
    }
    let mut data = context.data.write().await;
    let matches: &mut Vec<Match> = data.get_mut::<Matches>().unwrap();
    matches.push(new_match);
    write_to_file("matches.json", serde_json::to_string_pretty(matches).unwrap()).await;
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
    let mut parsed_match_id: Option<Uuid> = None;
    if let ApplicationCommandInteractionDataOptionValue::String(match_id) = option_one {
        if let Ok(id) = Uuid::from_str(match_id) {
            parsed_match_id = Some(id);
        } else {
            return String::from("Unable to parse match ID");
        }
    }
    let mut data = context.data.write().await;
    let matches: &mut Vec<Match> = data.get_mut::<Matches>().unwrap();
    let match_index = matches.iter().position(|m| m.id == parsed_match_id.unwrap());
    if let Some(index) = match_index {
        matches.remove(index);
    } else {
        return String::from("Could not find match");
    }
    write_to_file("matches.json", serde_json::to_string_pretty(matches).unwrap()).await;
    String::from("Successfully deleted match")
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

