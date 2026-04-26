use sqlx::query;
use tracing::info;

use crate::SQL;

#[derive(Debug, sqlx::Type, Clone)]
#[sqlx(type_name = "action_type", rename_all = "lowercase")]
pub enum ActionType {
    Warn,
    Kick,
    Ban,
    Softban,
    Mute,
    Unban,
    Unmute,
    Log,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionType::Warn => write!(f, "warn"),
            ActionType::Kick => write!(f, "kick"),
            ActionType::Ban => write!(f, "ban"),
            ActionType::Softban => write!(f, "softban"),
            ActionType::Mute => write!(f, "mute"),
            ActionType::Unban => write!(f, "unban"),
            ActionType::Unmute => write!(f, "unmute"),
            ActionType::Log => write!(f, "log"),
        }
    }
}
