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
use tracing::{debug, warn};

use crate::storage::Storage;

pub type MyBot = Throttle<CacheMe<DefaultParseMode<Bot>>>;
pub type MyDispatcher =
    Dispatcher<MyBot, teloxide::RequestError, teloxide::dispatching::DefaultKey>;

pub async fn start_bot(token: String, storage: Storage) {
    let bot = Bot::new(token.clone())
        .parse_mode(ParseMode::MarkdownV2)
        .cache_me()
        .throttle(Limits::default());

    let handler = Update::filter_message()
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

#[tracing::instrument(skip_all)]
async fn unauthorized_command_handler(
    bot: MyBot,
    msg: Message,
    cmd: UnauthorizedCommand,
    storage: Storage,
) -> ResponseResult<()> {
    debug!("unauthorized command: {:?}", cmd);
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
                let mention = match member.username {
                    Some(username) => format!("@{}", username),
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
async fn message_handler(
    bot: MyBot,
    msg: Message,
    storage: Storage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match msg.kind {
        MessageKind::NewChatMembers(members) => {
            for user in members.new_chat_members {
                storage.new_member(msg.chat.id, &user).await?;
            }
        }
        MessageKind::LeftChatMember(member) => {
            storage
                .delete_member(msg.chat.id, member.left_chat_member.id)
                .await?;
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
                            storage.new_member(msg.chat.id, &user).await?;
                        }
                        ChatMemberKind::Left | ChatMemberKind::Banned(_) => {
                            storage.delete_member(msg.chat.id, user.id).await?
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
