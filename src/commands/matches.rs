use crate::commands::maps::Map;
use crate::commands::matches::SeriesType::{Bo1, Bo3, Bo5};
use crate::commands::matches::VoteType::Veto;
use crate::commands::setup::get_dathost_config;
use crate::commands::team::Team;
use crate::Context;
use anyhow::Result;
use poise::command;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serenity::builder::{CreateActionRow, CreateButton};
use serenity::model::application::component::ButtonStyle;
use serenity::model::channel::ReactionType;
use sqlx::types::time::OffsetDateTime;
use sqlx::{FromRow, Type};
use sqlx::{PgExecutor, PgPool};
use std::str::FromStr;
use std::{env, fmt, i32};
use strum::EnumIter;

#[allow(unused)]
#[derive(Debug, FromRow)]
pub struct Match {
    pub id: i32,
    pub match_series: i32,
    pub map: i32,
    pub picked_by: i32,
    pub start_ct_team: i32,
    pub start_t_team: i32,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct NewMatch {
    pub map_id: i32,
    pub picked_by_role: i64,
    pub start_ct_team_role: Option<i64>,
    pub start_t_team_role: Option<i64>,
}

#[allow(unused)]
#[derive(Debug, FromRow)]
pub struct Server {
    pub id: i32,
    pub match_series: i32,
    pub server_id: String,
    pub hostname: String,
    pub game_port: i32,
    pub gotv_port: i32,
}

impl Server {
    pub async fn add(
        executor: impl PgExecutor<'_>,
        match_series: i32,
        server_id: &String,
        hostname: &String,
        game_port: i32,
        gotv_port: i32,
    ) -> Result<bool> {
        let result = sqlx::query!(
            "insert into servers (match_series, server_id, hostname, game_port, gotv_port)\
            values ($1, $2, $3, $4, $5)",
            match_series,
            server_id,
            hostname,
            game_port,
            gotv_port,
        )
        .execute(executor)
        .await?;
        let rows_affected = result.rows_affected() == 1;
        Ok(rows_affected)
    }
    async fn get_live(executor: impl PgExecutor<'_>) -> Result<Vec<Server>> {
        Ok(sqlx::query_as!(
            Server,
            "select s.* from servers s \
                join match_series ms on ms.id = s.match_series \
            where ms.completed_at is null",
        )
        .fetch_all(executor)
        .await?)
    }
}

#[derive(Deserialize, Debug)]
pub struct DathostSeriesResponse {
    pub matches: Vec<DathostMatch>,
}

#[derive(Deserialize, Debug)]
pub struct DathostMatch {
    pub id: String,
    pub map: String,
}

#[allow(unused)]
#[derive(Debug, FromRow)]
pub struct MatchSeries {
    pub id: i32,
    pub team_one: i32,
    pub team_two: i32,
    pub series_type: SeriesType,
    pub dathost_match: Option<String>,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, FromRow)]
pub struct VoteInfo {
    pub id: i32,
    pub match_series: i32,
    pub map: i32,
    #[sqlx(rename = "type")]
    pub vote_type: VoteType,
    pub team: i32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, Type, EnumIter,
)]
#[sqlx(rename_all = "lowercase", type_name = "series_type")]
pub enum SeriesType {
    Bo1,
    Bo3,
    Bo5,
}

impl fmt::Display for SeriesType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bo1 => write!(f, "bo1"),
            Bo3 => write!(f, "bo3"),
            Bo5 => write!(f, "bo5"),
        }
    }
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, Type,
)]
#[sqlx(rename_all = "lowercase", type_name = "vote_type")]
pub enum VoteType {
    Veto,
    Pick,
}

#[allow(unused)]
impl fmt::Display for VoteType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VoteType::Veto => write!(f, "Veto"),
            VoteType::Pick => write!(f, "Pick"),
        }
    }
}

#[derive(FromRow)]
#[allow(unused)]
pub struct MatchScore {
    pub id: i32,
    pub match_id: i32,
    pub team_one_score: i32,
    pub team_two_score: i32,
}

impl MatchScore {
    pub async fn add(executor: impl PgExecutor<'_>, match_series: i32) -> Result<bool> {
        let result = sqlx::query!(
            "
            insert into match_scores (match_id)
                    VALUES
                        ($1)
                    ",
            match_series,
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected() == 1)
    }
    async fn get_by_series(
        executor: impl PgExecutor<'_>,
        match_series: i32,
    ) -> Result<Vec<MatchScore>> {
        Ok(sqlx::query_as!(
            MatchScore,
            "select ms.*
                 from match_scores ms
                    join match m on ms.match_id = m.id
                    join match_series mts on m.match_series = mts.id
                where mts.id = $1 ",
            match_series,
        )
        .fetch_all(executor)
        .await?)
    }
    async fn get_in_progress(executor: impl PgExecutor<'_>) -> Result<Vec<MatchScore>> {
        Ok(sqlx::query_as!(
            MatchScore,
            "select ms.*
                 from match_scores ms
                    join match m on ms.match_id = m.id
                    join match_series mts on m.match_series = mts.id
                 where m.completed_at is null
                     and mts.completed_at is null
                 order by ms.id",
        )
        .fetch_all(executor)
        .await?)
    }
}

impl VoteInfo {
    pub async fn add(
        executor: impl PgExecutor<'_>,
        match_series: i32,
        map: i32,
        vote_type: VoteType,
        team: i32,
    ) -> Result<VoteInfo> {
        Ok(sqlx::query_as(
            "
            insert into vote_info (match_series, map, type, team)
                    VALUES
                        ($1, $2, $3, $4)
                    RETURNING *",
        )
        .bind(match_series)
        .bind(map)
        .bind(vote_type)
        .bind(team)
        .fetch_one(executor)
        .await?)
    }
    async fn get_by_match_series(
        executor: impl PgExecutor<'_>,
        match_series: i32,
    ) -> Result<Vec<VoteInfo>> {
        Ok(
            sqlx::query_as("select * from vote_info where match_series = $1 order by id")
                .bind(match_series)
                .fetch_all(executor)
                .await?,
        )
    }
}

impl MatchSeries {
    pub async fn create(
        executor: impl PgExecutor<'_>,
        team_one: i32,
        team_two: i32,
        series_type: SeriesType,
    ) -> Result<MatchSeries> {
        Ok(sqlx::query_as(
            "INSERT INTO match_series
                        (team_one, team_two, series_type, created_at)
                    VALUES
                        ($1, $2, $3, now())
                    RETURNING *",
        )
        .bind(team_one)
        .bind(team_two)
        .bind(series_type)
        .fetch_one(executor)
        .await?)
    }
    async fn get(executor: impl PgExecutor<'_>, id: i32) -> Result<Option<MatchSeries>> {
        Ok(
            sqlx::query_as(format!("select * from match_series where id = $1",).as_str())
                .bind(id)
                .fetch_optional(executor)
                .await?,
        )
    }
    async fn get_all(
        executor: impl PgExecutor<'_>,
        limit: u64,
        completed: bool,
    ) -> Result<Vec<MatchSeries>> {
        let completed_clause = if completed { "is not null" } else { "is null" };
        Ok(sqlx::query_as(
            format!(
                "select * from match_series
                    where completed_at {}
                 order by id desc limit $1",
                completed_clause
            )
            .as_str(),
        )
        .bind(limit as i64)
        .fetch_all(executor)
        .await?)
    }

    pub async fn update_dathost_match(
        &self,
        executor: impl PgExecutor<'_>,
        dathost_match_id: String,
    ) -> Result<bool> {
        let result = sqlx::query!(
            "UPDATE match_series SET dathost_match = $1 WHERE id = $2",
            dathost_match_id,
            self.id
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected() == 1)
    }
    pub async fn next_user_match(executor: impl PgExecutor<'_>, user: i64) -> Result<MatchSeries> {
        Ok(sqlx::query_as(
            "
                select ms.*
                from match_series ms
                    join teams t on t.id = ms.team_one or t.id = ms.team_two
                    join team_members tm on t.id = tm.team
                    join steam_ids si on si.discord = tm.member
                where si.discord = $1
                    and ms.completed_at is null
                order by ms.id",
        )
        .bind(user)
        .fetch_one(executor)
        .await?)
    }

    async fn get_all_by_user(
        executor: impl PgExecutor<'_>,
        limit: u64,
        user: u64,
        completed: bool,
    ) -> Result<Vec<MatchSeries>> {
        let completed_clause = if completed { "is not null" } else { "is null" };
        Ok(sqlx::query_as(
            format!(
                "select ms.*
                    from match_series ms
                    join teams t on t.id = ms.team_one or ms.team_two = t.id
                    join team_members tm on t.id = tm.team
                where tm.member = $2
                    and ms.completed_at {}
                order by ms.id desc
                limit $1",
                completed_clause
            )
            .as_str(),
        )
        .bind(limit as i64)
        .bind(user as i64)
        .fetch_all(executor)
        .await?)
    }
    pub async fn delete(executor: impl PgExecutor<'_>, id: i32) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM match_series where id = $1", id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    async fn get_in_progress(executor: impl PgExecutor<'_>) -> Result<Vec<MatchSeries>> {
        Ok(sqlx::query_as(
            "select ms.*
                 from match_series ms
                   join match m on m.match_series = ms.id
                   join match_scores mi on m.id = mi.match_id
                   join servers s on s.match_series = ms.id
                 where ms.completed_at is null
                   and m.completed_at is null
                 ",
        )
        .fetch_all(executor)
        .await?)
    }
    pub async fn veto_info(
        &self,
        pool: &PgPool,
        vote_info: Option<Vec<VoteInfo>>,
    ) -> Result<String> {
        let vote_info = if vote_info.is_none() {
            VoteInfo::get_by_match_series(pool, self.id).await?
        } else {
            vote_info.unwrap()
        };
        if vote_info.is_empty() {
            return Ok(String::from("This match has no veto info yet"));
        }
        let team_one = Team::get(pool, self.team_one).await.unwrap();
        let team_two = Team::get(pool, self.team_two).await.unwrap();
        let maps = Map::get_all(pool, false).await.unwrap();
        let mut info_string = String::from("```diff\n");
        let rows: String = vote_info
            .into_iter()
            .filter(|v| v.map > 0)
            .map(|v| {
                let mut row_str = String::new();
                let team_name = if v.team == team_one.id {
                    &team_one.name
                } else {
                    &team_two.name
                };
                let map_name = &maps.iter().find(|m| m.id == v.map).unwrap().name;
                if v.vote_type == Veto {
                    row_str.push_str(format!("- {} banned {}\n", team_name, map_name,).as_str());
                } else {
                    row_str.push_str(format!("+ {} picked {}\n", team_name, map_name,).as_str());
                }
                row_str
            })
            .collect();
        info_string.push_str(rows.as_str());
        info_string.push_str("```");
        Ok(info_string)
    }
}

impl Match {
    pub async fn create(
        executor: impl PgExecutor<'_>,
        match_series: i32,
        map: i32,
        picked_by: i32,
        start_ct_team: i32,
        start_t_team: i32,
    ) -> Result<Match> {
        Ok(sqlx::query_as(
            "INSERT INTO match 
                        (match_series, map, picked_by, start_ct_team, start_t_team)
                    VALUES
                        ($1, $2, $3, $4, $5)
                    RETURNING *",
        )
        .bind(match_series)
        .bind(map)
        .bind(picked_by)
        .bind(start_ct_team)
        .bind(start_t_team)
        .fetch_one(executor)
        .await?)
    }
    #[allow(dead_code)]
    pub(crate) async fn get_by_series(
        executor: impl PgExecutor<'_>,
        match_series: i32,
    ) -> Result<Vec<Match>> {
        Ok(sqlx::query_as!(
            Match,
            "select m.* from match_series ms \
                join match m on m.match_series = ms.id \
                where match_series = $1 \
                ",
            match_series
        )
        .fetch_all(executor)
        .await?)
    }
    async fn get_in_progress(executor: impl PgExecutor<'_>) -> Result<Vec<Match>> {
        Ok(sqlx::query_as(
            "select m.*
                 from match_series ms
                   join match m on m.match_series = ms.id
                   join match_scores mi on m.id = mi.match_id
                   join servers s on s.match_series = ms.id
                 where ms.completed_at is null
                   and m.completed_at is null
                 ",
        )
        .fetch_all(executor)
        .await?)
    }
}

#[command(
    slash_command,
    guild_only,
    subcommands("scheduled", "inprogress", "completed", "find")
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
    let info = MatchScore::get_in_progress(pool).await?;
    let match_series = MatchSeries::get_in_progress(pool).await?;
    let matches = Match::get_in_progress(pool).await?;
    if match_series.is_empty() || info.is_empty() {
        context.say("No matches in progress were found").await?;
        return Ok(());
    }
    let servers = Server::get_live(pool).await?;
    let mut teams = Vec::new();
    for m in &match_series {
        teams.push(Team::get(pool, m.team_one).await?);
        teams.push(Team::get(pool, m.team_two).await?);
    }
    let match_info: String = matches
        .into_iter()
        .map(|m| {
            let mut s = String::new();
            let m_info = info.iter().find(|i| i.match_id == m.id).unwrap();
            let team_one_score = m_info.team_one_score;
            let team_two_score = m_info.team_two_score;
            let series = match_series
                .iter()
                .find(|ms| ms.id == m.match_series)
                .unwrap();
            let team_one_role = teams.iter().find(|t| t.id == series.team_one).unwrap().role;
            let team_two_role = teams.iter().find(|t| t.id == series.team_two).unwrap().role;
            let server = servers
                .iter()
                .find(|s| s.match_series == series.id)
                .unwrap();
            s.push_str(format!("`#{}` ", m.id).as_str());
            s.push_str(format!("<@&{}> **`{}`**", &team_one_role, team_one_score).as_str());
            s.push_str(" - ");
            s.push_str(format!("**`{}`** <@&{}>", team_two_score, team_two_role).as_str());
            s.push_str("\n - ");
            s.push_str(
                format!(
                    "GOTV: ||`connect {}:{}`||\n",
                    server.hostname, server.gotv_port
                )
                .as_str(),
            );
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
    description_localized("en-US", "Show completed matches")
)]
pub(crate) async fn completed(context: Context<'_>) -> Result<()> {
    let pool = &context.data().pool;
    let matches = MatchSeries::get_all(pool, 20, true).await?;
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
pub(crate) async fn find(
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
    for (i, m) in matches.iter().enumerate() {
        let picked_by = Team::get(pool, m.picked_by).await?;
        let score = scores.iter().find(|i| i.match_id == m.id).unwrap();
        if score.team_one_score == 0 && score.team_two_score == 0 {
            continue;
        }
        played_match_ids.push(m.id);
        s.push_str(
            format!(
                "{}. `{}` ",
                i + 1,
                maps.iter().find(|map| map.id == m.map).unwrap().name,
            )
            .as_str(),
        );
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
            _ => {
                let dathost_matches =
                    get_series_dathost_matches(&series.dathost_match.as_ref().unwrap())
                        .await?
                        .into_iter()
                        .take(played_match_ids.len())
                        .collect();
                create_demo_link_row_series(&series.dathost_match.unwrap(), dathost_matches)
            }
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

async fn get_series_dathost_matches(dathost_series_id: &String) -> Result<Vec<DathostMatch>> {
    let dathost_config = get_dathost_config();
    let client = Client::new();
    let matches: Vec<DathostMatch> = client
        .get(format!(
            "https://dathost.net/api/0.1/match-series/{}",
            dathost_series_id
        ))
        .basic_auth(dathost_config.user, Some(&dathost_config.password))
        .send()
        .await?
        .json::<DathostSeriesResponse>()
        .await?
        .matches;
    Ok(matches)
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
    matches: Vec<DathostMatch>,
) -> Option<CreateActionRow> {
    let Ok(bucket_url) = env::var("BUCKET_URL") else {
        return None;
    };
    let mut ar = CreateActionRow::default();
    for (i, m) in matches.iter().enumerate() {
        let link_btn = get_series_demo_btn(&m.map, bucket_url.to_string(), series_id, i + 1);
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
