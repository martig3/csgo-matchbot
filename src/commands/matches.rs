use crate::Context;
use anyhow::Result;
use matchbot_core::maps::*;
use matchbot_core::matches::SeriesType::Bo1;
use matchbot_core::matches::SeriesType::Bo3;
use matchbot_core::matches::SeriesType::Bo5;
use matchbot_core::matches::*;
use matchbot_core::team::*;
use poise::command;
use serenity::builder::{CreateActionRow, CreateButton};
use serenity::model::application::component::ButtonStyle;
use serenity::model::channel::ReactionType;
use sqlx::PgPool;
use std::env;
use std::i32;

#[command(
    slash_command,
    guild_only,
    subcommands("scheduled", "inprogress", "completed", "info")
)]
pub(crate) async fn matches(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Show your scheduled matches")
)]
pub(crate) async fn scheduled(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;
    let matches = MatchSeries::get_all_by_user(pool, 20, context.author().id.0, false).await?;
    if matches.is_empty() {
        context.say("No matches were found").await?;
        return Ok(());
    }
    let teams = Team::get_all(pool).await?;
    let match_info: String = matches
        .into_iter()
        .map(|m| {
            let mut s = String::new();
            let team_one_name = &teams.iter().find(|t| t.id == m.team_one).unwrap().name;
            let team_two_name = &teams.iter().find(|t| t.id == m.team_two).unwrap().name;
            s.push_str(format!("`id: {}` ", m.id).as_str());
            s.push_str(format!("{}", &team_one_name).as_str());
            s.push_str(" vs ");
            s.push_str(format!("{}", &team_two_name).as_str());
            s.push_str("\n");
            s
        })
        .collect();
    context.say(match_info).await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Show all matches in progress & GOTV info")
)]
pub(crate) async fn inprogress(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;
    let match_series = MatchSeries::get_in_progress(pool).await?;
    if match_series.is_empty() {
        context.say("No matches in progress were found").await?;
        return Ok(());
    }
    let servers = Server::get_live(pool).await?;
    let mut resp_str = String::new();
    for series in &match_series {
        match series.series_type {
            Bo1 => resp_str.push_str(&match_inprogress_info(pool, series, &servers).await?),
            Bo3 => resp_str.push_str(&series_inprogress_info(pool, series, &servers).await?),
            Bo5 => resp_str.push_str(&series_inprogress_info(pool, series, &servers).await?),
        }
    }
    context.say(resp_str).await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Show completed matches")
)]
pub(crate) async fn completed(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;
    let matches = MatchSeries::get_all(pool, 20, true, None).await?;
    if matches.is_empty() {
        context.say("No matches were found").await?;
        return Ok(());
    }
    let teams = Team::get_all(pool).await?;
    let mut s = String::new();
    for m in matches {
        let scores = MatchScore::get_by_series(pool, m.id).await?;
        let (team_one_score, team_two_score) = get_series_score(&scores, m.series_type);
        if team_one_score == 0 && team_two_score == 0 {
            continue;
        }
        let team_one_name = &teams.iter().find(|t| t.id == m.team_one).unwrap().name;
        let team_two_name = &teams.iter().find(|t| t.id == m.team_two).unwrap().name;
        s.push_str(format!("`#{}` ", m.id).as_str());
        s.push_str(format!("{} **`{}`**", &team_one_name, team_one_score).as_str());
        s.push_str(" - ");
        s.push_str(format!("**`{}`** {}", team_two_score, &team_two_name).as_str());
        s.push_str("\n");
    }
    context.say(s).await?;
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Show info for a match")
)]
pub(crate) async fn info(
    context: Context<'_>,
    #[description = "Match number"] match_id: i32,
) -> Result<()> {
    let pool = &context.data().pool;
    let series = MatchSeries::get(pool, match_id).await?;
    let Some(series) = series else {
        context
            .say(format!("Could not find match with id: `{}`", match_id))
            .await?;
        return Ok(());
    };
    let team_one = Team::get(pool, series.team_one).await?;
    let team_two = Team::get(pool, series.team_two).await?;
    let matches = Match::get_by_series(pool, match_id).await?;
    let maps = Map::get_all(pool, false).await?;
    let scores = MatchScore::get_by_series(pool, match_id).await?;
    let (team_one_score, team_two_score) = get_series_score(&scores, series.series_type);
    let mut s = format!("**{}** `{}`", &team_one.name, team_one_score);
    s.push_str(" - ");
    s.push_str(format!("`{}` **{}**", team_two_score, &team_two.name).as_str());
    s.push_str("\n\n");
    let mut played_match_ids: Vec<i32> = Vec::new();
    let mut map_names = Vec::new();
    for (i, m) in matches.iter().enumerate() {
        let picked_by = Team::get(pool, m.picked_by).await?;
        let score = scores.iter().find(|i| i.match_id == m.id).unwrap();
        if score.team_one_score == 0 && score.team_two_score == 0 {
            continue;
        }
        played_match_ids.push(m.id);
        let map_name = &maps.iter().find(|map| map.id == m.map).unwrap().name;
        s.push_str(format!("{}. `{}` ", i + 1, map_name,).as_str());
        map_names.push(map_name);
        if series.series_type != Bo1 {
            s.push_str(format!("**`{}`**", score.team_one_score).as_str());
            s.push_str(" - ");
            s.push_str(format!("**`{}`**", score.team_two_score).as_str());
        }
        s.push_str(format!(" - picked by: **{}**\n", &picked_by.name,).as_str())
    }
    s.push_str(series.veto_info(pool, None).await?.as_str());
    let components = match series.completed_at {
        Some(_) => match &series.series_type {
            Bo1 => {
                let map_name = maps
                    .iter()
                    .find(|map| &map.id == &matches[0].map)
                    .unwrap()
                    .name
                    .clone();
                create_demo_link_row_bo1(series.dathost_match.unwrap(), &map_name)
            }
            _ => create_demo_link_row_series(&series.dathost_match.unwrap(), map_names),
        },
        None => None,
    };
    context
        .send(|b| {
            b.ephemeral(true);
            b.content(s);
            if let Some(row) = components {
                b.components(|c| c.add_action_row(row));
            }
            b
        })
        .await?;
    Ok(())
}

fn create_demo_link_row_bo1(dathost_id: String, map_name: &str) -> Option<CreateActionRow> {
    let Ok(bucket_url) = env::var("BUCKET_URL") else {
        return None;
    };
    let mut ar = CreateActionRow::default();
    let link_btn = get_demo_btn(map_name.to_string(), bucket_url, dathost_id);
    ar.add_button(link_btn);
    Some(ar)
}

fn create_demo_link_row_series(
    series_id: &String,
    map_names: Vec<&String>,
) -> Option<CreateActionRow> {
    let Ok(bucket_url) = env::var("BUCKET_URL") else {
        return None;
    };
    let mut ar = CreateActionRow::default();
    for (i, m) in map_names.iter().enumerate() {
        let link_btn = get_series_demo_btn(&m, bucket_url.to_string(), series_id, i + 1);
        ar.add_button(link_btn);
    }
    Some(ar)
}

fn get_demo_btn(map_name: String, bucket_url: String, dathost_id: String) -> CreateButton {
    let mut conn_button = CreateButton::default();
    conn_button.label(map_name);
    conn_button.style(ButtonStyle::Link);
    conn_button.emoji(ReactionType::Unicode("📺".parse().unwrap()));
    let url = format!("{}/{}.dem", bucket_url, dathost_id);
    conn_button.url(url);
    conn_button
}

fn get_series_demo_btn(
    map_name: &String,
    bucket_url: String,
    series_id: &String,
    index: usize,
) -> CreateButton {
    let mut conn_button = CreateButton::default();
    conn_button.label(map_name);
    conn_button.style(ButtonStyle::Link);
    conn_button.emoji(ReactionType::Unicode("📺".parse().unwrap()));
    let url = format!("{}/{}_{}.dem", bucket_url, series_id, index);
    conn_button.url(url);
    conn_button
}

pub fn get_series_score(scores: &Vec<MatchScore>, series_type: SeriesType) -> (i32, i32) {
    let team_one_score = match series_type {
        Bo1 => scores[0].team_one_score,
        _ => scores
            .iter()
            .filter(|m| m.team_one_score > 0 || m.team_two_score > 0)
            .fold(0, |a, s| {
                if s.team_one_score > s.team_two_score {
                    a + 1
                } else {
                    a
                }
            }),
    };
    let team_two_score = match series_type {
        Bo1 => scores[0].team_two_score,
        _ => scores
            .iter()
            .filter(|m| m.team_one_score > 0 || m.team_two_score > 0)
            .fold(0, |a, s| {
                if s.team_one_score < s.team_two_score {
                    a + 1
                } else {
                    a
                }
            }),
    };
    (team_one_score, team_two_score)
}

async fn match_inprogress_info(
    pool: &PgPool,
    series: &MatchSeries,
    servers: &Vec<Server>,
) -> Result<String> {
    let team_one_role = Team::get(pool, series.team_one).await?.role;
    let team_two_role = Team::get(pool, series.team_two).await?.role;
    let info = MatchScore::get_by_series(pool, series.id).await?;
    let matches = Match::get_by_series(pool, series.id).await?;
    let curr_match = matches.get(0).unwrap();
    let curr_score = info
        .iter()
        .find(|score| score.match_id == curr_match.id)
        .unwrap();
    let server = servers
        .iter()
        .find(|s| s.match_series == series.id)
        .unwrap();
    let mut s = String::new();
    s.push_str(format!("`#{}` ", series.id).as_str());
    s.push_str(format!("<@&{}> `{}`", &team_one_role, curr_score.team_one_score).as_str());
    s.push_str(" - ");
    s.push_str(format!("`{}` <@&{}>", curr_score.team_two_score, &team_two_role).as_str());
    s.push_str("\n - ");
    s.push_str(
        format!(
            "GOTV: ||`connect {}:{}`||\n",
            server.hostname, server.gotv_port
        )
        .as_str(),
    );
    Ok(s)
}
async fn series_inprogress_info(
    pool: &PgPool,
    series: &MatchSeries,
    servers: &Vec<Server>,
) -> Result<String> {
    let team_one_role = Team::get(pool, series.team_one).await?.role;
    let team_two_role = Team::get(pool, series.team_two).await?.role;
    let info = MatchScore::get_by_series(pool, series.id).await?;
    let matches = Match::get_by_series(pool, series.id).await?;
    let completed: Vec<&Match> = matches
        .iter()
        .filter(|m| m.completed_at.is_some())
        .collect();
    let in_progress: Vec<&Match> = matches
        .iter()
        .filter(|m| m.completed_at.is_none())
        .collect();
    let series_score = if completed.len() > 0 {
        completed.iter().fold((0, 0), |mut accum, item| {
            let i = info.iter().find(|i| i.match_id == item.id).unwrap();
            if i.team_one_score > i.team_two_score {
                accum.0 += 1;
            } else {
                accum.1 += 1;
            }
            accum
        })
    } else {
        (0, 0)
    };
    let curr_match = in_progress.get(0).unwrap();
    let curr_score = info
        .iter()
        .find(|score| score.match_id == curr_match.id)
        .unwrap();
    let server = servers
        .iter()
        .find(|s| s.match_series == series.id)
        .unwrap();
    let mut s = String::new();
    s.push_str(format!("`#{}` ", series.id).as_str());
    s.push_str(format!("<@&{}> `{}`", &team_one_role, curr_score.team_one_score).as_str());
    s.push_str(" - ");
    s.push_str(format!("`{}` <@&{}>", curr_score.team_two_score, &team_two_role).as_str());
    s.push_str(format!(" **({} - {})**", series_score.0, series_score.1).as_str());
    s.push_str("\n - ");
    s.push_str(
        format!(
            "GOTV: ||`connect {}:{}`||\n",
            server.hostname, server.gotv_port
        )
        .as_str(),
    );
    Ok(s)
}
