use diesel::PgConnection;
use r2d2::{PooledConnection};
use r2d2_diesel::ConnectionManager;
use serenity::model::prelude::{GuildContainer, Role, RoleId, User};
use serenity::model::prelude::application_command::{ApplicationCommandInteraction};
use serenity::prelude::Context;
use serenity::utils::MessageBuilder;
use match_bot::{create_match_setup_steps, create_series_maps, update_match_state};
use match_bot::models::MatchState::Completed;
use match_bot::models::{MatchSetupStep, NewMatchSetupStep, NewSeriesMap};
use crate::{Bo3, Config, DBConnectionPool, Maps, Match, Setup, SetupStep, State};
use crate::StepType::{Pick, Veto};

pub(crate) fn convert_steamid_to_64(steamid: &String) -> u64 {
    let steamid_split: Vec<&str> = steamid.split(":").collect();
    let y = steamid_split[1].parse::<i64>().unwrap();
    let z = steamid_split[2].parse::<i64>().unwrap();
    let steamid_64 = (z * 2) + y + 76561197960265728;
    return steamid_64 as u64;
}

pub(crate) async fn find_user_team_role(all_guild_roles: Vec<Role>, user: &User, context: &&Context) -> Result<Role, String> {
    let team_roles: Vec<Role> = all_guild_roles.into_iter().filter(|r| r.name.starts_with("Team")).collect();
    for team_role in team_roles {
        if let Ok(has_role) = user.has_role(&context.http, team_role.guild_id, team_role.id).await {
            if !has_role { continue; }
            return Ok(team_role);
        }
    }
    Err(String::from("User does not have a team role"))
}

pub(crate) async fn is_phase_allowed(context: &Context, msg: &ApplicationCommandInteraction, state: State) -> Result<(), String> {
    let mut data = context.data.write().await;
    let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
    if setup.current_phase != state {
        return Err(String::from("It is not the correct phase"));
    }
    let role_one = RoleId::from(setup.clone().team_one.unwrap() as u64).0;
    let role_two = RoleId::from(setup.clone().team_two.unwrap() as u64).0;
    if let Ok(has_role_one) = msg.user.has_role(&context.http, msg.guild_id.unwrap(), role_one).await {
        if let Ok(has_role_two) = msg.user.has_role(&context.http, msg.guild_id.unwrap(), role_two).await {
            if !has_role_one && !has_role_two {
                return Err(String::from("You are not part of either team currently running `/setup`"));
            }
        }
    }
    Ok(())
}


pub(crate) async fn user_team(context: &Context, msg: &ApplicationCommandInteraction) -> Result<u64, String> {
    let mut data = context.data.write().await;
    let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
    let role_one = RoleId::from(setup.clone().team_one.unwrap() as u64).0;
    let role_two = RoleId::from(setup.clone().team_two.unwrap() as u64).0;
    if let Ok(has_role_one) = msg.user.has_role(&context.http, msg.guild_id.unwrap(), role_one).await {
        if has_role_one { return Ok(role_one); }
        if let Ok(has_role_two) = msg.user.has_role(&context.http, msg.guild_id.unwrap(), role_two).await {
            if has_role_two { return Ok(role_two); }
        }
    }
    Err(String::from("You are not part of either team currently running `/setup`"))
}

pub(crate) async fn admin_check(context: &Context, inc_command: &ApplicationCommandInteraction) -> Result<String, String> {
    let data = context.data.write().await;
    let config: &Config = data.get::<Config>().unwrap();
    let role_name = context.cache.role(inc_command.guild_id.unwrap(), RoleId::from(config.discord.admin_role_id)).await.unwrap().name;
    return if inc_command.user.has_role(&context.http, GuildContainer::from(inc_command.guild_id.unwrap()), RoleId::from(config.discord.admin_role_id)).await.unwrap_or(false) {
        Ok(String::from("User has admin role"))
    } else {
        Err(MessageBuilder::new()
            .mention(&inc_command.user)
            .push(" this command requires the '")
            .push(role_name)
            .push("' role.")
            .build())
    };
}

pub(crate) async fn get_maps(context: &Context) -> Vec<String> {
    let data = context.data.write().await;
    let maps: &Vec<String> = data.get::<Maps>().unwrap();
    maps.clone()
}

pub(crate) async fn get_setup(context: &Context) -> Setup {
    let data = context.data.write().await;
    let setup_final: Setup = data.get::<Setup>().unwrap().clone();
    setup_final
}

pub(crate) async fn finish_setup(context: &Context) {
    let maps = get_maps(context).await;
    {
        let setup_final = get_setup(context).await;
        let mut match_setup_steps: Vec<NewMatchSetupStep> = Vec::new();
        let match_id = setup_final.match_id.unwrap();
        for v in setup_final.veto_pick_order {
            let step = NewMatchSetupStep {
                match_id: &match_id,
                step_type: &Veto,
                team_role_id: v.team_role_id,
                map: Option::from(v.map.unwrap()),
            };
            match_setup_steps.push(step);
        }
        let mut series_maps: Vec<NewSeriesMap> = Vec::new();
        let match_id = setup_final.match_id.unwrap();
        for m in setup_final.maps {
            let step = NewSeriesMap {
                match_id: &match_id,
                map: m.map,
                start_attack_team_role_id: m.start_attack_team_role_id,
                start_defense_team_role_id: m.start_defense_team_role_id,
            };
            series_maps.push(step);
        }
        let conn = get_pg_conn(&context).await;
        create_match_setup_steps(&conn, match_setup_steps);
        create_series_maps(&conn, series_maps);
        update_match_state(&conn, match_id, Completed);
    }
    {
        let mut data = context.data.write().await;
        let setup: &mut Setup = data.get_mut::<Setup>().unwrap();
        reset_setup(setup, maps);
    }
}


pub(crate) fn print_veto_info(setup_info: Vec<MatchSetupStep>, m: &Match) -> String {
    if setup_info.is_empty() {
        return String::from("_This match has no veto info yet_");
    }
    let mut resp = String::from("```diff\n");
    let veto: String = setup_info.clone().iter()
        .map(|v| {
            let mut veto_str = String::new();
            let team_name = if m.team_one_role_id == v.team_role_id { &m.team_one_name } else { &m.team_two_name };
            if v.step_type == Veto {
                veto_str.push_str(format!("- {} banned {}\n", team_name, v.map.clone().unwrap().to_uppercase()).as_str());
            } else {
                veto_str.push_str(format!("+ {} picked {}\n", team_name, v.map.clone().unwrap().to_uppercase()).as_str());
            }
            veto_str
        }).collect();
    resp.push_str(veto.as_str());
    resp.push_str("```");
    resp
}

pub(crate) fn print_match_info(m: &Match, show_id: bool) -> String {
    let mut schedule_str = String::new();
    if let Some(schedule) = &m.scheduled_time_str {
        schedule_str = format!(" > Scheduled: `{}`", schedule);
    }
    let mut row = String::new();
    row.push_str(format!("- {} vs {}{}", m.team_one_name, m.team_two_name, schedule_str).as_str());
    if m.note.is_some() {
        row.push_str(format!(" `{}`", m.note.clone().unwrap()).as_str());
    }
    row.push('\n');
    if show_id { row.push_str(format!("    _Match ID:_ `{}\n`", m.id).as_str()) }
    row
}

pub(crate) fn eos_printout(setup: Setup) -> String {
    let mut resp = String::from("\n\nSetup is completed. GLHF!\n\n");
    for (i, el) in setup.maps.iter().enumerate() {
        resp.push_str(format!("**{}. {}** - picked by: <@&{}>\n    _Defense start:_ <@&{}>\n    _Attack start:_ <@&{}>\n\n", i + 1, el.map.to_uppercase(), &el.picked_by, el.start_defense_team_role_id.clone().unwrap(), el.start_attack_team_role_id.clone().unwrap()).as_str())
    }
    resp
}


pub(crate) async fn handle_bo1_setup(_msg: &ApplicationCommandInteraction, setup: Setup) -> (Vec<SetupStep>, String) {
    let match_id = setup.match_id.unwrap();
    return (vec![
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
    ], format!("Best of 1 option selected. Starting map veto. <@&{}> bans first.\n", &setup.team_one.unwrap()));
}

pub(crate) async fn handle_bo3_setup(_msg: &ApplicationCommandInteraction, setup: Setup) -> (Vec<SetupStep>, String) {
    let match_id = setup.match_id.unwrap();
    return (vec![
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
    ], format!("Best of 3 option selected. Starting map veto. <@&{}> bans first.\n", &setup.team_one.unwrap()));
}

pub(crate) async fn handle_bo5_setup(_msg: &ApplicationCommandInteraction, setup: Setup) -> (Vec<SetupStep>, String) {
    let match_id = setup.match_id.unwrap();
    return (vec![
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Veto, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_two.unwrap() as i64, map: None },
        SetupStep { match_id, step_type: Pick, team_role_id: setup.clone().team_one.unwrap() as i64, map: None },
    ], format!("Best of 5 option selected. Starting map veto. <@&{}> bans first.\n", &setup.team_one.unwrap()));
}

pub(crate) fn reset_setup(setup: &mut Setup, maps: Vec<String>) {
    setup.team_one = None;
    setup.team_two = None;
    setup.maps = Vec::new();
    setup.vetoes = Vec::new();
    setup.maps_remaining = maps;
    setup.series_type = Bo3;
    setup.match_id = None;
    setup.veto_pick_order = Vec::new();
    setup.current_step = 0;
    setup.current_phase = State::Idle;
}

pub(crate) async fn get_pg_conn(context: &Context) -> PooledConnection<ConnectionManager<PgConnection>> {
    let data = context.data.write().await;
    let pool = data.get::<DBConnectionPool>().unwrap();
    pool.get().unwrap()
}
