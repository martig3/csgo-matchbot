use anyhow::Result;
use log::error;
use poise::command;
use poise::Modal;
use regex::Regex;
use sqlx::{FromRow, PgExecutor};
use steamid::{AccountType, Instance, SteamId, Universe};

use crate::Context;

trait ParseWithDefaults: Sized {
    fn parse<S: AsRef<str>>(value: S) -> Result<Self>;
}

impl ParseWithDefaults for SteamId {
    fn parse<S: AsRef<str>>(value: S) -> Result<Self> {
        let mut steamid =
            SteamId::parse_steam2id(value, AccountType::Individual, Instance::Desktop)?;
        steamid.set_universe(Universe::Public);
        Ok(steamid)
    }
}

#[derive(Debug, Modal)]
#[name = "Enter your SteamID"]
struct SteamIDModal {
    #[name = "SteamID"]
    #[placeholder = "i.e. STEAM:0:1:123456789"]
    #[min_length = 13]
    #[max_length = 19]
    steamid: String,
}

#[derive(Debug, FromRow)]
pub struct SteamUser {
    pub discord: i64,
    pub steam: i64,
}

impl SteamUser {
    pub async fn get_by_discord_id(
        executor: impl PgExecutor<'_>,
        discord_id: i64,
    ) -> Result<Option<SteamUser>> {
        Ok(sqlx::query_as!(
            SteamUser,
            "select * from steam_ids where discord = $1",
            discord_id
        )
        .fetch_optional(executor)
        .await?)
    }
    pub async fn get_by_team(executor: impl PgExecutor<'_>, team: i64) -> Result<Vec<SteamUser>> {
        Ok(sqlx::query_as!(
            SteamUser,
            "select si.*
                 from steam_ids si
                    join team_members tm on tm.member = si.discord
                    join teams t on t.id = tm.team
                 where t.role = $1",
            team
        )
        .fetch_all(executor)
        .await?)
    }
    async fn add(executor: impl PgExecutor<'_>, discord_id: i64, steamid: i64) -> Result<bool> {
        let result = sqlx::query!(
            "INSERT INTO steam_ids (discord, steam) VALUES ($1, $2)
                    ON CONFLICT (discord) DO UPDATE
                    SET steam = $2",
            discord_id,
            steamid,
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Set your SteamID")
)]
pub(crate) async fn steamid(
    context: Context<'_>,
    #[description = "Your SteamID"] steamid: String,
) -> Result<()> {
    let steam_id_regex = Regex::new("^STEAM_[0-5]:[01]:\\d+$").unwrap();
    if !steam_id_regex.is_match(&steamid) {
        context
            .say("Invalid SteamId format. SteamIds must follow this format: `STEAM_0:1:12345678`")
            .await?;
        return Ok(());
    }
    let steamid64 = SteamId::parse(&steamid);
    let Ok(steamid64) = steamid64 else {
        error!("Error parsing '{}'", &steamid);
        context.say(format!("Error parsing steamid '{}', contact an admin", steamid)).await?;
        return Ok(());
    };
    let pool = &context.data().pool;
    SteamUser::add(
        pool,
        context.author().id.0 as i64,
        u64::from(steamid64) as i64,
    )
    .await?;
    context.say(format!("Your SteamID has been set to the following Steam account: {} \
                                \nPlease verify this is the account you will be playing on, otherwise you will not be able to join a match server!",
                                     steamid64.community_link()))
            .await?;
    Ok(())
}
