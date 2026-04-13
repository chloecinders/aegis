use serde::{Deserialize, Serialize};
use serenity::all::MessageId;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracePoint {
    pub name: String,
    pub duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
