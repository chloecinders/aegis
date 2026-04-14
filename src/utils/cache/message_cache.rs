use std::collections::{HashMap, VecDeque};

use serenity::all::Message;

use crate::utils::cache::partials::PartialMessage;

pub struct MessageCache {
    sizes: HashMap<u64, usize>,
    messages: HashMap<u64, MessageQueue>,
    inserts: HashMap<u64, usize>,
}

impl MessageCache {
    pub fn new() -> Self {
        Self {
            sizes: HashMap::new(),
            messages: HashMap::new(),
            inserts: HashMap::new(),
        }
    }

    pub fn assign_count(&mut self, channel: u64, count: usize) {
        self.sizes.insert(channel, count);
        let entry = self
            .messages
            .entry(channel)
            .or_insert(MessageQueue::with_capacity(count));

        if entry.items.capacity() > count {
            while entry.items.len() > count {
                entry.pop();
            }

            entry.items.shrink_to(count);
        }
    }

    pub fn clear_inserts(&mut self) {
        self.inserts = self
            .inserts
            .clone()
            .into_keys()
            .map(|i| (i, 0_usize))
            .collect::<HashMap<_, _>>();
    }

    pub fn insert_message(&mut self, channel_id: u64, msg: Message) {
        let partial = PartialMessage::from(msg);
        self.insert(channel_id, partial);
    }

    pub fn insert(&mut self, channel_id: u64, message: PartialMessage) {
        *self.inserts.entry(channel_id).or_default() += 1;
        let queue_size = *self.sizes.entry(channel_id).or_insert(100);

        if queue_size == 0 {
            return;
        }

        let queue = self.messages.entry(channel_id).or_default();

        if queue.len() >= queue_size {
            queue.pop();
        }

        queue.insert(message);
    }

    pub fn get(&mut self, channel: u64, message: u64) -> Option<&PartialMessage> {
        let queue = self.messages.entry(channel).or_default();
        queue.get(message)
    }

    pub fn get_inserts(&self) -> HashMap<u64, usize> {
        self.inserts.clone()
    }

    pub fn get_sizes(&self) -> HashMap<u64, usize> {
        self.sizes.clone()
    }

    pub fn get_channel_len(&self, channel: u64) -> usize {
        self.messages
            .get(&channel)
            .map(|c| c.len())
            .unwrap_or_default()
    }

    pub fn byte_footprint(&self) -> usize {
        let mut size = std::mem::size_of::<Self>();
        size += self.sizes.capacity() * (std::mem::size_of::<u64>() + std::mem::size_of::<usize>());
        size +=
            self.inserts.capacity() * (std::mem::size_of::<u64>() + std::mem::size_of::<usize>());
        size += self.messages.capacity()
            * (std::mem::size_of::<u64>() + std::mem::size_of::<MessageQueue>());

        for queue in self.messages.values() {
            size += queue.items.capacity() * std::mem::size_of::<PartialMessage>();

            for msg in &queue.items {
                size += msg.byte_footprint() - std::mem::size_of::<PartialMessage>();
            }

            size += queue.index.capacity()
                * (std::mem::size_of::<u64>() + std::mem::size_of::<usize>());
        }
        size
    }
}

struct MessageQueue {
    pub items: VecDeque<PartialMessage>,
    index: HashMap<u64, usize>,
}

impl MessageQueue {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity),
            index: Default::default(),
        }
    }

    fn insert(&mut self, msg: PartialMessage) {
        let id = msg.id;

        if let Some(&idx) = self.index.get(&id) {
            self.items[idx] = msg;
        } else {
            self.index.insert(id, self.items.len());
            self.items.push_back(msg);
        }
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn get(&self, id: u64) -> Option<&PartialMessage> {
        self.index.get(&id).map(|&i| &self.items[i])
    }

    fn pop(&mut self) {
        if let Some(msg) = self.items.pop_front() {
            self.index.remove(&msg.id);

            for (i, m) in self.items.iter().enumerate() {
                self.index.insert(m.id, i);
            }
        }
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self {
            items: VecDeque::with_capacity(100),
            index: Default::default(),
        }
    }
}
