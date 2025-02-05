use core::fmt;

use teloxide::types::MessageKind;

pub(crate) struct DisplayMessageKind(pub(crate) MessageKind);

impl DisplayMessageKind {
    pub(crate) fn new(kind: &MessageKind) -> Self {
        Self(kind.clone())
    }
}

impl fmt::Display for DisplayMessageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = match self.0 {
            MessageKind::Common(_) => "Common",
            MessageKind::NewChatMembers(_) => "NewChatMembers",
            MessageKind::LeftChatMember(_) => "LeftChatMember",
            MessageKind::NewChatTitle(_) => "NewChatTitle",
            MessageKind::NewChatPhoto(_) => "NewChatPhoto",
            MessageKind::DeleteChatPhoto(_) => "DeleteChatPhoto",
            MessageKind::GroupChatCreated(_) => "GroupChatCreated",
            MessageKind::SupergroupChatCreated(_) => "SupergroupChatCreated",
            MessageKind::ChannelChatCreated(_) => "ChannelChatCreated",
            MessageKind::MessageAutoDeleteTimerChanged(_) => "MessageAutoDeleteTimerChanged",
            MessageKind::Pinned(_) => "Pinned",
            MessageKind::ChatShared(_) => "ChatShared",
            MessageKind::UsersShared(_) => "UsersShared",
            MessageKind::Invoice(_) => "Invoice",
            MessageKind::SuccessfulPayment(_) => "SuccessfulPayment",
            MessageKind::ConnectedWebsite(_) => "ConnectedWebsite",
            MessageKind::WriteAccessAllowed(_) => "WriteAccessAllowed",
            MessageKind::PassportData(_) => "PassportData",
            MessageKind::Dice(_) => "Dice",
            MessageKind::ProximityAlertTriggered(_) => "ProximityAlertTriggered",
            MessageKind::ForumTopicCreated(_) => "ForumTopicCreated",
            MessageKind::ForumTopicEdited(_) => "ForumTopicEdited",
            MessageKind::ForumTopicClosed(_) => "ForumTopicClosed",
            MessageKind::ForumTopicReopened(_) => "ForumTopicReopened",
            MessageKind::GeneralForumTopicHidden(_) => "GeneralForumTopicHidden",
            MessageKind::GeneralForumTopicUnhidden(_) => "GeneralForumTopicUnhidden",
            MessageKind::Giveaway(_) => "Giveaway",
            MessageKind::GiveawayCompleted(_) => "GiveawayCompleted",
            MessageKind::GiveawayCreated(_) => "GiveawayCreated",
            MessageKind::GiveawayWinners(_) => "GiveawayWinners",
            MessageKind::VideoChatScheduled(_) => "VideoChatScheduled",
            MessageKind::VideoChatStarted(_) => "VideoChatStarted",
            MessageKind::VideoChatEnded(_) => "VideoChatEnded",
            MessageKind::VideoChatParticipantsInvited(_) => "VideoChatParticipantsInvited",
            MessageKind::WebAppData(_) => "WebAppData",
            MessageKind::Empty {} => "Empty",
        };
        write!(f, "{}", variant)
    }
}
