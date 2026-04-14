use serde::{Deserialize, Serialize};
use serenity::all::MessageId;
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
