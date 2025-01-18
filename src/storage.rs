use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use teloxide::types::{ChatId, User, UserId};
use tracing::{debug, trace};

#[derive(Debug, Clone)]
pub struct Storage {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl Storage {
    pub async fn init(file: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("init storage");
        let opts = SqliteConnectOptions::new()
            .filename(file)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await?;
        Self::create_tables(&pool).await?;
        Ok(Self { pool })
    }

    async fn create_tables(pool: &sqlx::Pool<sqlx::Sqlite>) -> Result<(), sqlx::Error> {
        trace!("try create tables");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS members (
                chat_id INTEGER NOT NULL,
                user_id TEXT NOT NULL,
                username TEXT,
                first_name TEXT NOT NULL,
                last_name TEXT,
                PRIMARY KEY (chat_id, user_id)
            )",
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub(crate) async fn new_member(&self, chat_id: ChatId, user: &User) -> Result<(), sqlx::Error> {
        debug!(
            "adding member chat_id: {} user_id: {} username: {:?} first_name: {} last_name: {}",
            chat_id,
            user.id,
            &user.username.as_ref().map_or("<none>", |v| v),
            &user.first_name,
            &user.last_name.as_ref().map_or("<none>", |v| v),
        );
        sqlx::query(
            "INSERT OR IGNORE INTO members (chat_id, user_id, username, first_name, last_name) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(chat_id.0)
        .bind(user.id.to_string())
        .bind(user.username.clone())
        .bind(user.first_name.clone())
        .bind(user.last_name.clone())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn delete_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), sqlx::Error> {
        debug!("deleting member chat_id: {} user_id: {}", chat_id, user_id);
        sqlx::query("DELETE FROM members WHERE chat_id = ? AND user_id = ?")
            .bind(chat_id.0)
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(crate) async fn chat_members(&self, chat_id: ChatId) -> Result<Vec<Member>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM members WHERE chat_id = ?")
            .bind(chat_id.0)
            .fetch_all(&self.pool)
            .await
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct Member {
    pub(crate) chat_id: i64,
    pub(crate) user_id: String,
    pub(crate) username: Option<String>,
    pub(crate) first_name: String,
    pub(crate) last_name: Option<String>,
}
