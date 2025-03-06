use std::fmt::Write;

use teloxide::{
    adaptors::{throttle::Limits, CacheMe, DefaultParseMode, Throttle},
    prelude::*,
    types::{
        AllowedUpdate, BotCommandScope, ChatKind, ChatMember, ChatMemberKind, ChatPublic,
        LinkPreviewOptions, MessageId, MessageKind, ParseMode, PublicChatKind, Recipient,
        ReplyParameters, Update,
    },
    update_listeners::{webhooks, Polling},
    utils::{command::BotCommands, markdown},
    Bot,
};
use tracing::{debug, error, warn};

use crate::{config::BotConfig, storage::Storage, utils::DisplayMessageKind};

const GIT: &str = "github.com/lattenwald/tgpingerbot";

pub type MyBot = Throttle<CacheMe<DefaultParseMode<Bot>>>;
pub type MyDispatcher =
    Dispatcher<MyBot, teloxide::RequestError, teloxide::dispatching::DefaultKey>;

pub async fn start_bot(
    config: BotConfig,
    storage: Storage,
) -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::new(config.token.clone())
        .parse_mode(ParseMode::MarkdownV2)
        .cache_me()
        .throttle(Limits::default());

    if let Err(err) = bot
        .set_my_commands(UnauthorizedCommand::bot_commands())
        .scope(BotCommandScope::AllGroupChats)
        .await
    {
        error!("failed setting commands (default scope): {}", err);
    }

    if let Some(chat_id) = config.admin_id {
        let mut commands = Command::bot_commands();
        if let Ok(admin_chat) = bot.get_chat(ChatId(chat_id)).await {
            if admin_chat.is_group() {
                commands.extend(UnauthorizedCommand::bot_commands());
            }
        }
        if let Err(err) = bot
            .set_my_commands(commands)
            .scope(BotCommandScope::Chat {
                chat_id: Recipient::Id(ChatId(chat_id)),
            })
            .await
        {
            error!("failed setting commands (admin scope): {}", err);
        }
    }

    let handler = Update::filter_message()
        .branch(
            dptree::filter(move |msg: Message| {
                config
                    .admin_id
                    .is_some_and(|admin_id| admin_id == msg.chat.id.0)
            })
            .filter_command::<Command>()
            .endpoint(command_handler),
        )
        .branch(
            dptree::entry()
                .filter_command::<UnauthorizedCommand>()
                .endpoint(unauthorized_command_handler),
        )
        .branch(
            dptree::entry()
                .filter(|_msg: Message| true)
                .endpoint(message_handler),
        );

    let mut dispatcher = Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![storage])
        .build();

    let allowed_updates = vec![AllowedUpdate::Message, AllowedUpdate::ChatMember];
    if let Some(webhook_config) = config.webhook {
        let mut url = webhook_config.url.clone();
        {
            let mut path = url.path_segments_mut().unwrap();
            path.push(&format!("bot{}", config.token));
        }
        debug!("webhook url: {}", &url);
        let listener = webhooks::axum(
            bot.clone(),
            webhooks::Options::new(webhook_config.address, url),
        )
        .await?;
        let webhook_info = bot.get_webhook_info().await?;
        debug!("webhook info: {:#?}", webhook_info);
        let error_handler =
            LoggingErrorHandler::with_custom_text("An error from the update listener");
        dispatcher
            .dispatch_with_listener(listener, error_handler)
            .await;
    } else {
        bot.delete_webhook().send().await?;
        let listener = Polling::builder(bot)
            .allowed_updates(allowed_updates)
            .build();
        let error_handler =
            LoggingErrorHandler::with_custom_text("An error from the update listener");
        dispatcher
            .dispatch_with_listener(listener, error_handler)
            .await;
    };

    Ok(())
}

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "snake_case", description = "Общие команды:")]
enum UnauthorizedCommand {
    #[command(description = "id текущего чата")]
    Id,

    #[command(description = "пингануть всех")]
    Ping,

    #[command(description = "сколько тут юзеров, кого пингуем")]
    Count,

    #[command(description = "помощь")]
    Help,
}

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "snake_case", description = "Админские команды:")]
enum Command {
    #[command(description = "добавить пользователя", parse_with = "split")]
    AddUser(String, String),

    #[command(description = "миграция")]
    MigrateFrom(String),

    #[command(description = "чаты с пользователями")]
    Counts,

    #[command(description = "помощь")]
    Help,
}

#[tracing::instrument(skip_all)]
async fn unauthorized_command_handler(
    bot: MyBot,
    msg: Message,
    cmd: UnauthorizedCommand,
    storage: Storage,
) -> ResponseResult<()> {
    debug!("unauthorized command: {:?}", cmd);
    if let Some(ref from) = msg.from {
        let _ = storage.new_member(&msg.chat, from).await;
    }
    match cmd {
        UnauthorizedCommand::Id => {
            if let Err(err) = bot
                .send_message(msg.chat.id, format!("`{}`", msg.chat.id))
                .reply_parameters(ReplyParameters::new(msg.id))
                .await
            {
                warn!("failed sending message: {:?}", err);
            }
        }
        UnauthorizedCommand::Help => {
            let help = format!(
                "{}\n\n[{}](https://{})",
                markdown::escape(&UnauthorizedCommand::descriptions().to_string()),
                markdown::escape(GIT),
                markdown::escape(GIT),
            );
            reply(&bot, msg.chat.id, msg.id, &help).await;
        }
        UnauthorizedCommand::Ping => {
            let reply_to_msg_id = msg.reply_to_message().map(|msg| msg.id).unwrap_or(msg.id);
            let members = storage.chat_members(msg.chat.id).await.unwrap();

            let mut buf = if let Some(u) = msg.from.as_ref() {
                format!(
                    "{} вызывает\\!\n\n",
                    u.username.as_ref().map_or(
                        format!(
                            "[{}](tg://user?id={})",
                            markdown::escape(&u.full_name()),
                            u.id
                        ),
                        |n| { format!("@{}", markdown::escape(n)) }
                    )
                )
            } else {
                String::new()
            };

            let mut count = 0;
            let mut total = 0;
            for member in members {
                if member.is_bot {
                    continue;
                }
                if msg
                    .from
                    .as_ref()
                    .is_some_and(|f| f.id.0.to_string() == member.user_id)
                {
                    continue;
                }
                let mention = match member.username {
                    Some(username) => format!("@{}", markdown::escape(&username)),
                    None => format!(
                        "[{}](tg://user?id={})",
                        markdown::escape(&member.full_name()),
                        member.user_id
                    ),
                };
                let _ = write!(buf, " {}", mention);
                count += 1;
                total += 1;

                if count >= 40 {
                    reply(&bot, msg.chat.id, reply_to_msg_id, &buf).await;
                    buf.clear();
                    count = 0;
                }
            }
            if total == 0 {
                reply(
                    &bot,
                    msg.chat.id,
                    reply_to_msg_id,
                    "Тут нет никого, кроме нас",
                )
                .await;
            } else if count > 0 {
                reply(&bot, msg.chat.id, reply_to_msg_id, &buf).await;
            }
        }
        UnauthorizedCommand::Count => {
            let count = storage.chat_members_count(msg.chat.id).await.unwrap();
            reply(
                &bot,
                msg.chat.id,
                msg.id,
                &format!("В этом чате пингую `{}` пользователей", count),
            )
            .await;
        }
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
async fn command_handler(
    bot: MyBot,
    msg: Message,
    cmd: Command,
    storage: Storage,
) -> ResponseResult<()> {
    debug!("authorized command: {:?}", cmd);
    if let Some(ref from) = msg.from {
        let _ = storage.new_member(&msg.chat, from).await;
    }
    match cmd {
        Command::Help => {
            let help = format!(
                "{}\n\n{}\n\n[{}](https://{})",
                markdown::escape(&Command::descriptions().to_string()),
                markdown::escape(&UnauthorizedCommand::descriptions().to_string()),
                markdown::escape(GIT),
                markdown::escape(GIT),
            );
            reply(&bot, msg.chat.id, msg.id, &help).await;
        }
        Command::AddUser(chat_id, user_id) => {
            let Ok(chat_id) = chat_id.parse::<i64>().map(ChatId) else {
                reply(&bot, msg.chat.id, msg.id, "Неправильный id чата").await;
                return Ok(());
            };
            let Ok(user_id) = user_id.parse().map(UserId) else {
                reply(&bot, msg.chat.id, msg.id, "Неправильный id пользователя").await;
                return Ok(());
            };
            match check_member(&bot, &storage, chat_id, user_id).await {
                Ok(true) => {
                    reply(&bot, msg.chat.id, msg.id, "Пользователь есть в чате").await;
                    return Ok(());
                }
                Ok(false) => {
                    reply(&bot, msg.chat.id, msg.id, "Пользователь не найден").await;
                    return Ok(());
                }
                Err(err) => {
                    error!("failed checking member: {}", err);
                    reply(
                        &bot,
                        msg.chat.id,
                        msg.id,
                        &format!(
                            "Ошибка\n```\n{}\n```",
                            markdown::escape(&format!("{:#?}", err))
                        ),
                    )
                    .await;
                    return Ok(());
                }
            }
        }
        Command::MigrateFrom(version) => match version.as_str() {
            "0.1" => match storage.old_members().await {
                Ok(members) => {
                    let mut migrated = 0;
                    for member in members {
                        let chat_id = ChatId(member.chat_id);
                        let Ok(user_id) = member.user_id.parse().map(UserId) else {
                            reply(&bot, msg.chat.id, msg.id, "Неправильный id пользователя").await;
                            continue;
                        };
                        match check_member(&bot, &storage, chat_id, user_id).await {
                            Ok(true) => {
                                migrated += 1;
                            }
                            Ok(false) => {}
                            Err(err) => {
                                error!("failed checking member: {}", err);
                                reply(
                                    &bot,
                                    msg.chat.id,
                                    msg.id,
                                    &format!(
                                        "Ошибка проверки `chat\\_id={}` `user\\_id={}`\n```\n{}\n```",
                                        member.chat_id,
                                        markdown::escape(&member.user_id),
                                        markdown::escape(&format!("{:#?}", err))
                                    ),
                                ).await;
                            }
                        }
                    }

                    reply(
                        &bot,
                        msg.chat.id,
                        msg.id,
                        &format!("Успешно мигрировано {} пользователей", migrated),
                    )
                    .await;
                }
                Err(err) => {
                    error!("failed getting old members: {}", err);
                    reply(
                        &bot,
                        msg.chat.id,
                        msg.id,
                        &format!(
                            "Ошибка получения старых пользователей\n```\n{}\n```",
                            markdown::escape(&format!("{:#?}", err))
                        ),
                    )
                    .await;
                }
            },
            _ => {
                reply(
                    &bot,
                    msg.chat.id,
                    msg.id,
                    &format!("Нет миграции с версии `{}`", markdown::escape(&version)),
                )
                .await;
                return Ok(());
            }
        },
        Command::Counts => {
            let chats_with_counts = storage.chats_with_counts().await.unwrap();
            let mut buf = "Юзеров по всем чатам:\n\n".to_string();
            for (chat_id, title, count) in chats_with_counts {
                let _ = writeln!(
                    buf,
                    "`{}`: `{}`\n",
                    markdown::escape(&title.unwrap_or(chat_id.to_string())),
                    count
                );
            }
            reply(&bot, msg.chat.id, msg.id, &buf).await;
        }
    }
    Ok(())
}

/// Returns true if member is in chat, false otherwise
#[tracing::instrument(skip_all)]
async fn check_member(
    bot: &MyBot,
    storage: &Storage,
    chat_id: ChatId,
    user_id: UserId,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let ChatMember { user, kind } = bot.get_chat_member(chat_id, user_id).await?;
    let chat = bot.get_chat(chat_id).await?;
    let ChatKind::Public(ChatPublic {
        kind: ref public_chat_kind,
        ..
    }) = chat.kind
    else {
        return Ok(false);
    };
    if let PublicChatKind::Channel(_) = public_chat_kind {
        return Ok(false);
    }

    let span = tracing::span!(
        tracing::Level::DEBUG,
        "check_member",
        kind = ?kind
    );
    let _enter = span.enter();
    match kind {
        ChatMemberKind::Owner(_) | ChatMemberKind::Administrator(_) | ChatMemberKind::Member => {
            storage.new_member(&chat, &user).await?;
            return Ok(true);
        }
        ChatMemberKind::Left | ChatMemberKind::Banned(_) => {
            storage.delete_member(chat_id, user.id).await?;
            return Ok(false);
        }
        ChatMemberKind::Restricted(_) => {}
    }
    Ok(false)
}

#[tracing::instrument(skip_all, fields(msg_kind = %DisplayMessageKind::new(&msg.kind)))]
async fn message_handler(bot: MyBot, msg: Message, storage: Storage) -> ResponseResult<()> {
    match msg.kind {
        MessageKind::NewChatMembers(members) => {
            for user in members.new_chat_members {
                if let Err(err) = storage.new_member(&msg.chat, &user).await {
                    error!("failed adding member: {}", err);
                }
            }
        }
        MessageKind::LeftChatMember(member) => {
            if let Err(err) = storage
                .delete_member(msg.chat.id, member.left_chat_member.id)
                .await
            {
                error!("failed deleting member: {}", err);
            }
        }
        _ => {
            for user in msg.mentioned_users() {
                if let Ok(ChatMember { user, kind }) =
                    bot.get_chat_member(msg.chat.id, user.id).await
                {
                    match kind {
                        ChatMemberKind::Owner(_)
                        | ChatMemberKind::Administrator(_)
                        | ChatMemberKind::Member => {
                            if let Err(err) = storage.new_member(&msg.chat, &user).await {
                                error!("failed adding member: {}", err);
                            }
                        }
                        ChatMemberKind::Left | ChatMemberKind::Banned(_) => {
                            if let Err(err) = storage.delete_member(msg.chat.id, user.id).await {
                                error!("failed deleting member: {}", err);
                            }
                        }
                        ChatMemberKind::Restricted(_) => {}
                    }
                }
            }
        }
    }
    Ok(())
}

async fn reply(bot: &MyBot, chat_id: ChatId, msg_id: MessageId, text: &str) {
    debug!("sending message: {}", text);
    if let Err(err) = bot
        .send_message(chat_id, text)
        .reply_parameters(ReplyParameters {
            message_id: msg_id,
            ..Default::default()
        })
        .link_preview_options(LinkPreviewOptions {
            is_disabled: true,
            url: None,
            prefer_small_media: true,
            prefer_large_media: false,
            show_above_text: false,
        })
        .await
    {
        warn!("failed sending message: {:?}", err);
    }
}
