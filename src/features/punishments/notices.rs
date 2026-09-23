use std::time::Duration;

use serenity::all::{GenericChannelId, Message, MessageId};

use crate::domain::Snowflake;
use crate::platform::cache::Cache;
use crate::platform::ui::marks::Marks;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub channel: GenericChannelId,
    pub message: MessageId,
}

impl From<&Message> for Location {
    fn from(message: &Message) -> Self {
        Self {
            channel: message.channel_id,
            message: message.id,
        }
    }
}

pub struct Notices {
    replies: Cache<Snowflake, Marks>,
    notices: Cache<Snowflake, Location>,
}

impl Notices {
    pub fn new(window: Duration) -> Self {
        Self {
            replies: Cache::new(2048, Some(window)),
            notices: Cache::new(2048, Some(window)),
        }
    }

    pub fn remember_reply(&self, invocation: Snowflake, marks: Marks) {
        self.replies.insert(invocation, marks);
    }

    pub fn reply(&self, invocation: Snowflake) -> Option<Marks> {
        self.replies.get(&invocation)
    }

    pub fn remember_notice(&self, invocation: Snowflake, at: Location) {
        self.notices.insert(invocation, at);
    }

    pub fn notice(&self, invocation: Snowflake) -> Option<Location> {
        self.notices.get(&invocation)
    }

    pub fn sweep(&self) {
        self.replies.sweep();
        self.notices.sweep();
    }
}
