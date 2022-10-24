use anyhow::Result;
use sqlx::{FromRow, PgExecutor};

#[derive(Debug, FromRow, Clone)]
pub struct Map {
    pub id: i32,
    pub name: String,
    pub disabled: bool,
}

impl Map {
    pub async fn get(executor: impl PgExecutor<'_>, map_id: i32) -> Result<Map> {
        Ok(
            sqlx::query_as!(Map, "select * from maps where id = $1", map_id,)
                .fetch_one(executor)
                .await?,
        )
    }
    pub async fn get_all(executor: impl PgExecutor<'_>, only_enabled: bool) -> Result<Vec<Map>> {
        if only_enabled {
            Ok(
                sqlx::query_as!(Map, "select * from maps where disabled is false",)
                    .fetch_all(executor)
                    .await?,
            )
        } else {
            Ok(sqlx::query_as!(Map, "select * from maps",)
                .fetch_all(executor)
                .await?)
        }
    }
}
