use serenity::all::MessageId;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TracePoint {
    pub name: String,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct CommandTrace {
    pub message_id: MessageId,
    pub command_name: String,
    pub points: Vec<TracePoint>,
    pub total_duration: Duration,
    pub success: bool,
    pub error: Option<String>,
}

impl CommandTrace {
    pub fn byte_footprint(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.command_name.capacity()
            + self.points.capacity() * std::mem::size_of::<TracePoint>()
            + self.points.iter().map(|p| p.name.capacity()).sum::<usize>()
            + self.error.as_ref().map(|e| e.capacity()).unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct TraceCache {
    order: VecDeque<MessageId>,
    map: HashMap<MessageId, CommandTrace>,
    limit: usize,
}

impl TraceCache {
    pub fn new() -> Self {
        Self {
            order: VecDeque::with_capacity(100),
            map: HashMap::with_capacity(100),
            limit: 100,
        }
    }

    pub fn insert(&mut self, trace: CommandTrace) {
        if self.order.len() >= self.limit {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            }
        }
        self.order.push_back(trace.message_id);
        self.map.insert(trace.message_id, trace);
    }

    pub fn get(&self, id: MessageId) -> Option<&CommandTrace> {
        self.map.get(&id)
    }

    pub fn byte_footprint(&self) -> usize {
        let mut size = std::mem::size_of::<Self>();
        size += self.order.capacity() * std::mem::size_of::<MessageId>();
        size += self.map.capacity()
            * (std::mem::size_of::<MessageId>() + std::mem::size_of::<CommandTrace>());
        for trace in self.map.values() {
            size += trace.byte_footprint() - std::mem::size_of::<CommandTrace>();
        }
        size
    }
}
