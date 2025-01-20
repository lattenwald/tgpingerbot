use teloxide::types::{User, UserId};

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
pub(crate) struct MemberV01 {
    pub(crate) chat_id: i64,
    pub(crate) user_id: String,
    pub(crate) is_bot: bool,
    pub(crate) username: Option<String>,
    pub(crate) first_name: String,
    pub(crate) last_name: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) is_premium: bool,
    pub(crate) added_to_attachment_menu: bool,
}

impl From<MemberV01> for User {
    fn from(value: MemberV01) -> Self {
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
