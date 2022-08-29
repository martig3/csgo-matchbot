use crate::dathost_models::DathostServerDuplicateResponse;
use crate::{Config, DBConnectionPool, DathostConfig, Setup, SetupStep};
use csgo_matchbot::models::SeriesType::Bo5;
use csgo_matchbot::models::StepType::{Pick, Veto};
use csgo_matchbot::models::{
    Match, MatchServer, MatchSetupStep, MatchState, NewMatchSetupStep, NewSeriesMap, SeriesType,
    StepType,
};
use csgo_matchbot::{
    create_match_setup_steps, create_series_maps, get_fresh_token, get_map_pool, get_match_servers,
    get_user_by_discord_id, update_match_state, update_token,
};
use std::time::Duration;
use diesel::PgConnection;
use r2d2::PooledConnection;
use r2d2_diesel::ConnectionManager;
use reqwest::{Client, Error, Response};
use serenity::builder::{CreateActionRow, CreateButton, CreateSelectMenu, CreateSelectMenuOption};
use serenity::futures::StreamExt;
use serenity::model::application::component::ButtonStyle;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::message_component::MessageComponentInteraction;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::channel::{Message, ReactionType};
use serenity::model::id::GuildId;
use serenity::model::prelude::{GuildContainer, Role, RoleId, User};
use serenity::prelude::Context;
use serenity::utils::MessageBuilder;
use std::collections::HashMap;
use std::sync::Arc;
use urlencoding::encode;

pub(crate) fn convert_steamid_to_64(steamid: &str) -> u64 {
    let steamid_split: Vec<&str> = steamid.split(':').collect();
    let y = steamid_split[1].parse::<i64>().unwrap();
    let z = steamid_split[2].parse::<i64>().unwrap();
    ((z * 2) + y + 76561197960265728) as u64
}

pub(crate) async fn find_user_team_role(
    all_guild_roles: Vec<Role>,
    user: &User,
    context: &&Context,
) -> Result<Role, String> {
    let team_roles: Vec<Role> = all_guild_roles
        .into_iter()
        .filter(|r| r.name.starts_with("Team"))
        .collect();
    for team_role in team_roles {
        if let Ok(has_role) = user
            .has_role(&context.http, team_role.guild_id, team_role.id)
            .await
        {
            if !has_role {
                continue;
            }
            return Ok(team_role);
        }
    }
    Err(String::from("User does not have a team role"))
}

pub(crate) async fn user_team_author(
    context: &Context,
    setup: &Setup,
    msg: &Arc<MessageComponentInteraction>,
) -> Result<u64, String> {
    let role_one = RoleId::from(setup.clone().team_one.unwrap() as u64).0;
    let role_two = RoleId::from(setup.clone().team_two.unwrap() as u64).0;
    if let Ok(has_role_one) = msg
        .user
        .has_role(&context.http, msg.guild_id.unwrap(), role_one)
        .await
    {
        if has_role_one {
            return Ok(role_one);
        }
        if let Ok(has_role_two) = msg
            .user
            .has_role(&context.http, msg.guild_id.unwrap(), role_two)
            .await
        {
            if has_role_two {
                return Ok(role_two);
            }
        }
    }
    Err(String::from(
        "You are not part of either team currently running `/setup`",
    ))
}

pub(crate) async fn admin_check(
    context: &Context,
    inc_command: &ApplicationCommandInteraction,
) -> Result<String, String> {
    let data = context.data.write().await;
    let config: &Config = data.get::<Config>().unwrap();
    let role_name = context
        .cache
        .role(
            inc_command.guild_id.unwrap(),
            RoleId::from(config.discord.admin_role_id),
        )
        .unwrap()
        .name;
    return if inc_command
        .user
        .has_role(
            &context.http,
            GuildContainer::from(inc_command.guild_id.unwrap()),
            RoleId::from(config.discord.admin_role_id),
        )
        .await
        .unwrap_or(false)
    {
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
    let conn = get_pg_conn(context).await;
    let map_pool = get_map_pool(&conn);
    map_pool.into_iter().map(|m| m.name).collect()
}

pub(crate) async fn get_servers(context: &Context) -> Vec<MatchServer> {
    let conn = get_pg_conn(context).await;
    get_match_servers(&conn)
}

pub(crate) async fn finish_setup(context: &Context, setup_final: &Setup) {
    let mut match_setup_steps: Vec<NewMatchSetupStep> = Vec::new();
    let match_id = setup_final.match_id.unwrap();
    let conn = get_pg_conn(context).await;
    for v in &setup_final.veto_pick_order {
        let step = NewMatchSetupStep {
            match_id,
            step_type: v.step_type,
            team_role_id: v.team_role_id,
            map: Option::from(v.map.clone().unwrap()),
        };
        match_setup_steps.push(step);
    }
    let mut series_maps: Vec<NewSeriesMap> = Vec::new();
    let match_id = setup_final.match_id.unwrap();
    for m in &setup_final.maps {
        let step = NewSeriesMap {
            match_id,
            map: m.map.clone(),
            picked_by_role_id: m.picked_by,
            start_attack_team_role_id: m.start_attack_team_role_id,
            start_defense_team_role_id: m.start_defense_team_role_id,
        };
        series_maps.push(step);
    }
    create_match_setup_steps(&conn, match_setup_steps.clone());
    create_series_maps(&conn, series_maps.clone());
    update_match_state(&conn, match_id, MatchState::Completed);
}

pub(crate) fn print_veto_info(setup_info: &Vec<MatchSetupStep>, m: &Match) -> String {
    if setup_info.is_empty() {
        return String::from("_This match has no veto info yet_");
    }
    let mut resp = String::from("```diff\n");
    let veto: String = setup_info
        .clone()
        .iter()
        .map(|v| {
            let mut veto_str = String::new();
            let team_name = if m.team_one_role_id == v.team_role_id {
                &m.team_one_name
            } else {
                &m.team_two_name
            };
            if v.map.is_none() {
                return veto_str;
            }
            if v.step_type == Veto {
                veto_str.push_str(
                    format!(
                        "- {} banned {}\n",
                        team_name,
                        v.map.clone().unwrap().to_lowercase()
                    )
                    .as_str(),
                );
            } else {
                veto_str.push_str(
                    format!(
                        "+ {} picked {}\n",
                        team_name,
                        v.map.clone().unwrap().to_lowercase()
                    )
                    .as_str(),
                );
            }
            veto_str
        })
        .collect();
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
    row.push_str(
        format!(
            "- {} vs {}{}",
            m.team_one_name, m.team_two_name, schedule_str
        )
        .as_str(),
    );
    if m.note.is_some() {
        row.push_str(format!(" `{}`", m.note.clone().unwrap()).as_str());
    }
    row.push('\n');
    if show_id {
        row.push_str(format!("    _Match ID:_ `{}\n`", m.id).as_str())
    }
    row
}

pub(crate) fn eos_printout(setup: &Setup) -> String {
    let mut resp = String::from("\n\nSetup is completed. GLHF!\n\n");
    for (i, el) in setup.maps.iter().enumerate() {
        resp.push_str(
            format!(
                "**{}. {}** - picked by: <@&{}>\n    _CT start:_ <@&{}>\n    _T start:_ <@&{}>\n\n",
                i + 1,
                el.map.to_lowercase(),
                &el.picked_by,
                el.start_defense_team_role_id.unwrap(),
                el.start_attack_team_role_id.unwrap()
            )
            .as_str(),
        )
    }
    resp
}

pub async fn no_team_resp(context: &Context, mci: &Arc<MessageComponentInteraction>) {
    mci.create_interaction_response(&context, |r| {
        r.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|d| {
                d.ephemeral(true)
                    .content("You are not part of either team currently setting up a match")
            })
    })
    .await
    .unwrap();
}

pub(crate) async fn handle_bo1_setup(setup: Setup) -> (Vec<SetupStep>, String) {
    let match_id = setup.match_id.unwrap();
    (
        vec![
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
        ],
        format!(
            "Best of 1 option selected. Starting map veto. <@&{}> bans first.\n",
            &setup.team_two.unwrap()
        ),
    )
}

pub(crate) async fn handle_bo3_setup(setup: Setup) -> (Vec<SetupStep>, String) {
    let match_id = setup.match_id.unwrap();
    (
        vec![
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
        ],
        format!(
            "Best of 3 option selected. Starting map veto. <@&{}> bans first.\n",
            &setup.team_one.unwrap()
        ),
    )
}

pub(crate) async fn handle_bo5_setup(setup: Setup) -> (Vec<SetupStep>, String) {
    let match_id = setup.match_id.unwrap();
    (
        vec![
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Veto,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_two.unwrap() as i64,
                map: None,
            },
            SetupStep {
                match_id,
                step_type: Pick,
                team_role_id: setup.team_one.unwrap() as i64,
                map: None,
            },
        ],
        format!(
            "Best of 5 option selected. Starting map veto. <@&{}> bans first.\n",
            &setup.team_one.unwrap()
        ),
    )
}

pub(crate) async fn get_pg_conn(
    context: &Context,
) -> PooledConnection<ConnectionManager<PgConnection>> {
    let data = context.data.write().await;
    let pool = data.get::<DBConnectionPool>().unwrap();
    pool.get().unwrap()
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

pub fn create_server_conn_button_row(url: &String, gotv_url: &String, show_cmds: bool) -> CreateActionRow {
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

pub fn create_map_action_row(map_list: Vec<String>, step_type: &StepType) -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut menu = CreateSelectMenu::default();
    menu.custom_id("map_select");
    menu.placeholder(format!("Select map to {}", step_type));
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

pub fn create_server_action_row(server_list: &Vec<MatchServer>) -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut menu = CreateSelectMenu::default();
    menu.custom_id("server_select");
    menu.placeholder("Select server");
    let mut options = Vec::new();
    for server in server_list {
        options.push(create_menu_option(&server.region_label, &server.server_id))
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

pub async fn start_server(
    context: &Context,
    guild_id: GuildId,
    setup: &mut Setup,
) -> Result<DathostServerDuplicateResponse, Error> {
    println!("{:#?}", setup);
    let dathost_config = get_config(context).await.dathost;
    let conn = get_pg_conn(context).await;
    let client = Client::new();
    println!("duplicating server");
    let dupl_url = format!(
        "https://dathost.net/api/0.1/game-servers/{}/duplicate",
        encode(&setup.server_id.clone().unwrap())
    );
    let resp = client
        .post(dupl_url)
        .basic_auth(&dathost_config.user, Some(&dathost_config.password))
        .send()
        .await
        .unwrap()
        .json::<DathostServerDuplicateResponse>()
        .await;
    let resp = resp?;
    let server_id = resp.id.clone();

    let mut gslt = get_fresh_token(&conn);
    println!("setting gslt '{}'", &gslt.token);
    let gslt_resp = client
        .put(format!(
            "https://dathost.net/api/0.1/game-servers/{}",
            encode(&server_id.to_string())
        ))
        .form(&[
            ("name", format!("match-server-{}", setup.match_id.unwrap())),
            (
                "csgo_settings.steam_game_server_login_token",
                gslt.token.clone(),
            ),
        ])
        .basic_auth(&dathost_config.user, Some(&dathost_config.password))
        .send()
        .await
        .unwrap();
    if gslt_resp.status() == 200 {
        gslt.in_use = true;
        update_token(&conn, gslt);
    }
    let users: Vec<User> = context
        .http
        .get_guild_members(*guild_id.as_u64(), None, None)
        .await
        .unwrap()
        .iter()
        .map(|u| u.user.clone())
        .collect();
    let mut team_one_users = Vec::new();
    let mut team_two_users = Vec::new();
    for u in users {
        if u.has_role(&context, guild_id, setup.team_one.unwrap() as u64)
            .await
            .unwrap()
        {
            team_one_users.push(u.clone());
        }
        if u.has_role(&context, guild_id, setup.team_two.unwrap() as u64)
            .await
            .unwrap()
        {
            team_two_users.push(u.clone());
        }
    }
    println!("1: {:#?}", team_one_users);
    println!("2: {:#?}", team_two_users);
    let conn = get_pg_conn(context).await;
    setup.team_one_conn_str = Some(map_steamid_strings(team_one_users, &conn));
    setup.team_two_conn_str = Some(map_steamid_strings(team_two_users, &conn));
    println!(
        "starting match\nteam1 '{}'\nteam2: '{}'",
        setup.clone().team_one_conn_str.unwrap(),
        setup.clone().team_two_conn_str.unwrap()
    );
    let start_resp = match setup.series_type {
        SeriesType::Bo1 => start_match(server_id, setup, client, &dathost_config).await,
        SeriesType::Bo3 => start_series_match(server_id, setup, client, &dathost_config).await,
        SeriesType::Bo5 => start_series_match(server_id, setup, client, &dathost_config).await,
    };
    if let Err(err) = start_resp {
        eprintln!("{:#?}", err);
        return Err(err);
    }
    println!("{:#?}", start_resp.unwrap().text().await.unwrap());
    Ok(resp)
}

pub async fn start_match(
    server_id: String,
    setup: &Setup,
    client: Client,
    dathost_config: &DathostConfig,
) -> Result<Response, Error> {
    let start_match_url = String::from("https://dathost.net/api/0.1/matches");
    let team_ct: String;
    let team_t: String;
    let team_ct_name: String;
    let team_t_name: String;
    let new_match = setup.maps[0].clone();
    if setup.maps[0].start_defense_team_role_id == setup.team_one {
        team_ct = setup.team_one_conn_str.clone().unwrap();
        team_ct_name = setup.team_one_name.clone();
        team_t = setup.team_two_conn_str.clone().unwrap();
        team_t_name = setup.team_two_name.clone();
    } else {
        team_ct = setup.team_two_conn_str.clone().unwrap();
        team_ct_name = setup.team_two_name.clone();
        team_t = setup.team_one_conn_str.clone().unwrap();
        team_t_name = setup.team_one_name.clone();
    }
    println!("starting match request...");
    client
        .post(&start_match_url)
        .form(&[
            ("game_server_id", &&server_id),
            ("map", &&new_match.map),
            ("team1_name", &&team_t_name),
            ("team2_name", &&team_ct_name),
            ("team1_steam_ids", &&team_t),
            ("team2_steam_ids", &&team_ct),
            ("enable_pause", &&String::from("true")),
            ("enable_tech_pause", &&String::from("true")),
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
) -> Result<Response, Error> {
    let start_match_url = String::from("https://dathost.net/api/0.1/match-series");
    let team_one = setup.team_one_conn_str.clone().unwrap();
    let team_one_name = setup.team_one_name.clone();
    let team_two = setup.team_two_conn_str.clone().unwrap();
    let team_two_name = setup.team_two_name.clone();
    let mut params: HashMap<&str, &str> = HashMap::new();
    let team_map = HashMap::from([
        (setup.team_one.unwrap(), "team1"),
        (setup.team_two.unwrap(), "team2"),
    ]);
    let mut num_maps = "3";
    params.insert("game_server_id", server_id.as_str());
    params.insert("enable_pause", "true");
    params.insert("enable_tech_pause", "true");
    params.insert("team1_name", team_one_name.as_str());
    params.insert("team2_name", team_two_name.as_str());
    params.insert("team1_steam_ids", team_one.as_str());
    params.insert("team2_steam_ids", team_two.as_str());
    params.insert("map1", setup.maps[0].map.as_str());
    params.insert(
        "map1_start_ct",
        team_map
            .get(&setup.maps[0].start_defense_team_role_id.unwrap())
            .unwrap(),
    );
    params.insert("map2", setup.maps[1].map.as_str());
    params.insert(
        "map2_start_ct",
        team_map
            .get(&setup.maps[1].start_defense_team_role_id.unwrap())
            .unwrap(),
    );
    params.insert("map3", setup.maps[2].map.as_str());
    params.insert(
        "map3_start_ct",
        team_map
            .get(&setup.maps[2].start_defense_team_role_id.unwrap())
            .unwrap(),
    );
    if setup.series_type == Bo5 {
        num_maps = "5";
        params.insert("map4", setup.maps[3].map.as_str());
        params.insert(
            "map4_start_ct",
            team_map
                .get(&setup.maps[3].start_defense_team_role_id.unwrap())
                .unwrap(),
        );
        params.insert("map5", setup.maps[4].map.as_str());
        params.insert(
            "map5_start_ct",
            team_map
                .get(&setup.maps[4].start_defense_team_role_id.unwrap())
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

pub fn map_steamid_strings(
    users: Vec<User>,
    conn: &PooledConnection<ConnectionManager<PgConnection>>,
) -> String {
    let mut str: String = users
        .iter()
        .map(|u| get_user_by_discord_id(conn, &i64::from(u.id)).steam_id)
        .map(|mut s| {
            s.replace_range(6..7, "1");
            s
        })
        .map(|s| format!("{},", s))
        .collect();
    str.remove(str.len() - 1);
    str
}

pub async fn create_conn_message(
    context: &Context,
    msg: &Message,
    server: DathostServerDuplicateResponse,
    setup: &Setup,
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

    let mut m = msg.channel_id
        .send_message(&context, |m| m.content(eos_printout(setup))
        .components(|c|
            c.add_action_row(
                create_server_conn_button_row(&t_url, &t_gotv_url, true)
            )),
    ).await.unwrap();
    let mut cib = m
        .await_component_interactions(&context)
        .timeout(Duration::from_secs(60 * 5))
        .build();
    loop {
        let opt = cib.next().await;
        match opt {
            Some(mci) => {
                mci.create_interaction_response(&context, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(|d| {
                        d.ephemeral(true).content(format!("Console: ||`connect {}`||\nGOTV: ||`connect {}`||", &game_url, &gotv_url))
                    })
                }).await.unwrap();
            }
            None => {
                // remove console cmds interaction on timeout
                m.edit(&context, |m|
                    m.content(eos_printout(&setup))
                        .components(|c|
                            c.add_action_row(create_server_conn_button_row(&t_url, &t_gotv_url, false)
                            )
                        ),
                ).await.unwrap();
                return;
            }
        }
    }
}

pub async fn get_config(context: &Context) -> Config {
    let data = context.data.write().await;
    let config: &Config = data.get::<Config>().unwrap();
    config.clone()
}
