use std::fmt::Write;

use teloxide::{
    adaptors::{throttle::Limits, CacheMe, DefaultParseMode, Throttle},
    prelude::*,
    types::{
        AllowedUpdate, ChatMember, ChatMemberKind, MessageId, MessageKind, ParseMode,
        ReplyParameters, Update,
    },
    update_listeners::Polling,
    utils::{command::BotCommands, markdown},
    Bot,
};
use tracing::{debug, error, warn};

use crate::storage::Storage;

pub type MyBot = Throttle<CacheMe<DefaultParseMode<Bot>>>;
pub type MyDispatcher =
    Dispatcher<MyBot, teloxide::RequestError, teloxide::dispatching::DefaultKey>;

pub async fn start_bot(token: String, storage: Storage, admin_id: i64) {
    let bot = Bot::new(token.clone())
        .parse_mode(ParseMode::MarkdownV2)
        .cache_me()
        .throttle(Limits::default());

    let handler = Update::filter_message()
        .branch(
            dptree::filter(move |msg: Message| msg.chat.id.0 == admin_id)
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

    let polling = Polling::builder(bot)
        .allowed_updates(vec![AllowedUpdate::Message, AllowedUpdate::ChatMember])
        .build();

    let error_handler = LoggingErrorHandler::with_custom_text("An error from the update listener");

    dispatcher
        .dispatch_with_listener(polling, error_handler)
        .await;
}

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "snake_case", description = "Доступные команды:")]
enum UnauthorizedCommand {
    #[command(description = "id текущего чата")]
    Id,

    #[command(description = "пингануть всех")]
    Ping,

    #[command(description = "помощь")]
    Help,
}

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "snake_case", description = "Доступные команды:")]
enum Command {
    #[command(description = "добавить пользователя", parse_with = "split")]
    AddUser(String, String),

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
        let _ = storage.new_member(msg.chat.id, from).await;
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
            reply(
                &bot,
                msg.chat.id,
                msg.id,
                &markdown::escape(&UnauthorizedCommand::descriptions().to_string()),
            )
            .await;
        }
        UnauthorizedCommand::Ping => {
            let reply_to_msg_id = msg.reply_to_message().map(|msg| msg.id).unwrap_or(msg.id);
            let members = storage.chat_members(msg.chat.id).await.unwrap();

            let mut buf = String::new();
            let mut count = 0;
            if members.is_empty() {
                reply(&bot, msg.chat.id, reply_to_msg_id, "А нет никого").await;
                return Ok(());
            }
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
                        markdown::escape(&member.first_name),
                        member.user_id
                    ),
                };
                let _ = write!(buf, " {}", mention);
                count += 1;

                if count >= 40 {
                    reply(&bot, msg.chat.id, reply_to_msg_id, &buf).await;
                    buf.clear();
                    count = 0;
                }
            }

            if count > 0 {
                reply(&bot, msg.chat.id, reply_to_msg_id, &buf).await;
            }
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
    match cmd {
        Command::Help => {
            let help = format!(
                "*Авторизованные команды:*\n\n{}\n\n*Неавторизованные команды:*\n\n{}",
                Command::descriptions(),
                UnauthorizedCommand::descriptions()
            );
            reply(&bot, msg.chat.id, msg.id, &markdown::escape(&help)).await;
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
            if let Ok(ChatMember { user, kind }) = bot.get_chat_member(chat_id, user_id).await {
                match kind {
                    ChatMemberKind::Owner(_)
                    | ChatMemberKind::Administrator(_)
                    | ChatMemberKind::Member => {
                        if let Err(err) = storage.new_member(msg.chat.id, &user).await {
                            error!("failed adding member: {}", err);
                            reply(&bot, msg.chat.id, msg.id, "Пользователь не добавлен").await;
                        } else {
                            reply(&bot, msg.chat.id, msg.id, "Пользователь добавлен").await;
                        }
                    }
                    ChatMemberKind::Left | ChatMemberKind::Banned(_) => {
                        if let Err(err) = storage.delete_member(msg.chat.id, user.id).await {
                            error!("failed deleting member: {}", err);
                            reply(&bot, msg.chat.id, msg.id, "Пользователь не удален").await;
                        } else {
                            reply(&bot, msg.chat.id, msg.id, "Пользователь удален").await;
                        }
                    }
                    ChatMemberKind::Restricted(_) => {}
                }
            } else {
                reply(&bot, msg.chat.id, msg.id, "Пользователь не найден").await;
            }
        }
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
async fn message_handler(bot: MyBot, msg: Message, storage: Storage) -> ResponseResult<()> {
    match msg.kind {
        MessageKind::NewChatMembers(members) => {
            for user in members.new_chat_members {
                if let Err(err) = storage.new_member(msg.chat.id, &user).await {
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
                            if let Err(err) = storage.new_member(msg.chat.id, &user).await {
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
        .await
    {
        warn!("failed sending message: {:?}", err);
    }
}
