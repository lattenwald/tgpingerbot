use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use teloxide::types::{Chat, ChatId, ChatKind, ChatPublic, PublicChatKind, User, UserId};
use tracing::{debug, info, trace};

#[derive(Debug, Clone)]
pub struct Storage {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

mod v01;

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
            "CREATE TABLE IF NOT EXISTS chat_members (
                chat_id INTEGER NOT NULL,
                user_id TEXT NOT NULL,
                PRIMARY KEY (chat_id, user_id)
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (
                user_id TEXT NOT NULL,
                is_bot BOOLEAN NOT NULL DEFAULT FALSE,
                username TEXT,
                first_name TEXT NOT NULL,
                last_name TEXT,
                language TEXT,
                is_premium BOOLEAN NOT NULL DEFAULT FALSE,
                added_to_attachment_menu BOOLEAN NOT NULL DEFAULT FALSE,
                PRIMARY KEY (user_id)
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chats (
                chat_id INTEGER NOT NULL,
                title TEXT,
                username TEXT,
                PRIMARY KEY (chat_id)
            )",
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub(crate) async fn new_member(&self, chat: &Chat, user: &User) -> Result<(), sqlx::Error> {
        let ChatKind::Public(ChatPublic {
            kind: ref public_chat_kind,
            ..
        }) = chat.kind
        else {
            debug!("chat is not public");
            return Ok(());
        };
        if let PublicChatKind::Channel(_) = public_chat_kind {
            debug!("chat is channel");
            return Ok(());
        }
        debug!("new member chat_id: {} user_id: {}", chat.id, user.id);

        let user_id = user.id.to_string();
        sqlx::query(
            "INSERT
            INTO users (
                user_id, is_bot, username, first_name,
                last_name, language, is_premium, added_to_attachment_menu
                )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (user_id)
            DO UPDATE SET
                is_bot = EXCLUDED.is_bot,
                username = EXCLUDED.username,
                first_name = EXCLUDED.first_name,
                last_name = EXCLUDED.last_name,
                language = EXCLUDED.language,
                is_premium = EXCLUDED.is_premium,
                added_to_attachment_menu = EXCLUDED.added_to_attachment_menu",
        )
        .bind(&user_id)
        .bind(user.is_bot)
        .bind(&user.username)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(&user.language_code)
        .bind(user.is_premium)
        .bind(user.added_to_attachment_menu)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT
            INTO chats (chat_id, title, username)
            VALUES (?, ?, ?)
            ON CONFLICT (chat_id)
            DO UPDATE SET
                title = EXCLUDED.title,
                username = EXCLUDED.username",
        )
        .bind(chat.id.0)
        .bind(chat.title())
        .bind(chat.username())
        .execute(&self.pool)
        .await?;

        let result =
            sqlx::query("INSERT OR IGNORE INTO chat_members (chat_id, user_id) VALUES (?, ?)")
                .bind(chat.id.0)
                .bind(&user_id)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() > 0 {
            info!(
                "added member chat_id: {} user_id: {} username: {:?} first_name: {} last_name: {}",
                chat.id,
                user.id,
                &user.username.as_ref().map_or("<none>", |v| v),
                &user.first_name,
                &user.last_name.as_ref().map_or("<none>", |v| v)
            );
        }
        Ok(())
    }

    pub(crate) async fn delete_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), sqlx::Error> {
        debug!("delete member chat_id: {} user_id: {}", chat_id, user_id);
        let result = sqlx::query("DELETE FROM chat_members WHERE chat_id = ? AND user_id = ?")
            .bind(chat_id.0)
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() > 0 {
            info!("deleted member chat_id: {} user_id: {}", chat_id, user_id);
        }
        Ok(())
    }

    pub(crate) async fn chat_members(&self, chat_id: ChatId) -> Result<Vec<Member>, sqlx::Error> {
        sqlx::query_as("SELECT u.* FROM chat_members cm JOIN users u ON cm.user_id = u.user_id WHERE cm.chat_id = ? AND NOT(u.is_bot)")
            .bind(chat_id.0)
            .fetch_all(&self.pool)
            .await
    }

    pub(crate) async fn chat_members_count(&self, chat_id: ChatId) -> Result<u64, sqlx::Error> {
        sqlx::query("SELECT COUNT(*) FROM chat_members cm JOIN users u ON cm.user_id = u.user_id WHERE cm.chat_id = ? AND NOT(u.is_bot)")
            .bind(chat_id.0)
            .fetch_one(&self.pool)
            .await
            .map(|row| row.get(0))
    }

    pub(crate) async fn chats_with_counts(
        &self,
    ) -> Result<Vec<(i64, Option<String>, u64)>, sqlx::Error> {
        sqlx::query_as("SELECT c.chat_id, c.title, COUNT(cm.user_id) FROM chats c JOIN chat_members cm ON c.chat_id = cm.chat_id LEFT JOIN users u ON cm.user_id = u.user_id WHERE NOT u.is_bot ORDER BY c.chat_id")
            .fetch_all(&self.pool)
            .await
    }

    pub(crate) async fn old_members(&self) -> Result<Vec<v01::MemberV01>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM members")
            .fetch_all(&self.pool)
            .await
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct Member {
    pub(crate) user_id: String,
    pub(crate) is_bot: bool,
    pub(crate) username: Option<String>,
    pub(crate) first_name: String,
    pub(crate) last_name: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) is_premium: bool,
    pub(crate) added_to_attachment_menu: bool,
}

impl From<Member> for User {
    fn from(value: Member) -> Self {
        User {
            id: UserId(value.user_id.parse().unwrap()),
            is_bot: value.is_bot,
            username: value.username,
            first_name: value.first_name,
            last_name: value.last_name,
            language_code: value.language,
            is_premium: value.is_premium,
            added_to_attachment_menu: value.added_to_attachment_menu,
        }
    }
}

impl Member {
    pub(crate) fn full_name(&self) -> String {
        match &self.last_name {
            Some(last_name) => format!("{0} {1}", self.first_name, last_name),
            None => self.first_name.clone(),
        }
    }
}
