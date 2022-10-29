use crate::Data;
use anyhow::{Error, Result};
use log::error;
use poise::Modal;
use poise::{command, ApplicationContext};
use sqlx::{FromRow, PgExecutor};
use steamid::{AccountType, Instance, SteamId, Universe};

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
    description_localized("en-US", "Set your SteamID")
)]
pub(crate) async fn steamid(context: ApplicationContext<'_, Data, Error>) -> Result<()> {
    let data: SteamIDModal = SteamIDModal::execute(context).await?;
    let steamid_str = data.steamid.trim();
    let steamid64 = SteamId::parse(steamid_str);
    if let Ok(steamid64) = steamid64 {
        SteamUser::add(
            &context.data.pool,
            context.interaction.user().id.0 as i64,
            u64::from(steamid64) as i64,
        )
        .await?;
        context.interaction
            .unwrap()
            .create_followup_message(
            &context.discord.http,
            |m| m
                    .ephemeral(true)
                    .content(format!("Your steamID has been set to the following Steam account: {} \
                                \nPlease verify this is the account you will be playing on, otherwise you will not be able to join a match server!",
                                     steamid64.community_link())))
            .await?;
    } else {
        error!("Error parsing '{}'", steamid_str)
    }
    Ok(())
}
