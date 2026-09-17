use std::sync::{Arc, Mutex};

use crate::domain::Snowflake;
use crate::platform::cache::lru::Lru;
use crate::platform::discord::partial::PartialMessage;

pub type Cached = Arc<PartialMessage>;

const CHANNELS: usize = 512;
const PER_CHANNEL: usize = 100;

pub struct Recent {
    channels: Mutex<Lru<Snowflake, Lru<Snowflake, Cached>>>,
}

impl Default for Recent {
    fn default() -> Self {
        Self::new()
    }
}

impl Recent {
    pub fn new() -> Self {
        Self {
            channels: Mutex::new(Lru::new(CHANNELS)),
        }
    }

    pub fn remember(&self, message: Cached) {
        let Ok(mut channels) = self.channels.lock() else {
            return;
        };

        let (channel, id) = (message.channel_id, message.id);

        if let Some(known) = channels.get_mut(&channel) {
            known.insert(id, message);

            return;
        }

        let mut opened = Lru::new(PER_CHANNEL);

        opened.insert(id, message);
        channels.insert(channel, opened);
    }

    pub fn take(&self, channel: Snowflake, message: Snowflake) -> Option<Cached> {
        self.channels
            .lock()
            .ok()?
            .get_mut(&channel)?
            .remove(&message)
    }

    pub fn peek(&self, channel: Snowflake, message: Snowflake) -> Option<Cached> {
        self.channels
            .lock()
            .ok()?
            .peek(&channel)?
            .peek(&message)
            .map(Arc::clone)
    }
}
