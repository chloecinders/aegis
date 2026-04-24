use core::fmt;
use std::{collections::HashMap, fmt::Debug, iter::Peekable, pin::Pin, sync::Arc, vec::IntoIter};

use crate::{
    event_handler::{CommandError, Handler, MissingArgumentError},
    lexer::Token,
};
use serenity::{
    all::{Context, GuildChannel, Member, Message, Permissions, User},
    async_trait,
};

/// An error enum used by transformers
#[allow(clippy::large_enum_variant)]
pub enum TransformerError {
    /// Used when a general command/transformer error happened
    CommandError(CommandError),
    /// Used when there is no actual input to transform
    MissingArgumentError(MissingArgumentError),
}

/// The return type of transformer functions
pub type TransformerReturn<'a> =
    Pin<Box<dyn Future<Output = Result<Token, TransformerError>> + Send + 'a>>;
/// The exact type of transformer functions
pub type TransformerFn = dyn for<'a> Fn(&'a Context, &'a Message, &'a mut Peekable<IntoIter<Token>>) -> TransformerReturn<'a>
    + Send
    + Sync;
/// Transformer function wrapped in an Arc
pub type TransformerFnArc = Arc<TransformerFn>;

/// Defines the values which can be passed to a command
#[derive(Debug, Clone)]
#[allow(non_camel_case_types)]
pub enum CommandArgument {
    String(String),
    User(User),
    // ManyUsers(Vec<User>), // may be used in the future
    Member(Member),
    Duration(chrono::Duration),
    None,
    i32(i32),
    GuildChannel(GuildChannel),
    bool(bool),
}

/// Defines a commands syntax, implementing a type and a visual representation of the syntax
pub enum CommandSyntax {
    /// Consume the remaining text as a String argument
    Consume(&'static str),
    User(&'static str, bool),
    Member(&'static str, bool),
    Channel(&'static str, bool),
    String(&'static str, bool),
    Duration(&'static str, bool),
    /// A string but with a more appropriate example
    Reason(&'static str),
    Number(&'static str, bool),
    /// Consume the remaining text as a set of filters (used for the purge command)
    Filters,
    Or(Box<CommandSyntax>, Box<CommandSyntax>),
}

/// Command parameter which changes command behaviour, the original input gets cut out before being processed further into tokens/arguments
pub struct CommandParameter<'a> {
    pub name: &'a str,
    /// A short description for the parameter
    pub short: &'a str,
    /// The transformer function to use when processing the input into a CommandArgument
    pub transformer: &'a TransformerFn,
    /// A long description for the parameter
    pub desc: &'a str,
}

#[derive(PartialEq, Eq, Hash)]
pub enum CommandCategory {
    Misc,
    Utilities,
    Moderation,
    Admin,
    Developer,
}

impl fmt::Display for CommandCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                CommandCategory::Misc => "Misc",
                CommandCategory::Utilities => "Utilities",
                CommandCategory::Moderation => "Moderation",
                CommandCategory::Admin => "Admin",
                CommandCategory::Developer => "Developer",
            }
        )
    }
}

impl CommandSyntax {
    /// Returns the definition of a type, meaning something like `<(argument name): (argument type)>`: [target: User]
    pub fn get_def(&self) -> String {
        let (inner, required) = match self {
            Self::Consume(name) | Self::Reason(name) => (format!("...[{name}]"), None),
            Self::Or(a, b) => (format!("({} || {})", a.get_def(), b.get_def()), None),
            Self::User(name, opt) => (format!("{name}: Discord User"), Some(opt)),
            Self::Member(name, opt) => (format!("{name}: Discord Member"), Some(opt)),
            Self::String(name, opt) => (format!("{name}: String"), Some(opt)),
            Self::Duration(name, opt) => (format!("{name}: Duration"), Some(opt)),
            Self::Number(name, opt) => (format!("{name}: Number"), Some(opt)),
            Self::Filters => (String::from("...[filters]"), None),
            Self::Channel(name, opt) => (format!("{name}: Channel"), Some(opt)),
        };

        if let Some(is_required) = required {
            if *is_required {
                format!("<{inner}>")
            } else {
                format!("[{inner}]")
            }
        } else {
            inner
        }
    }

    /// A short example of what input could be passed
    pub fn get_example(&self) -> String {
        match self {
            CommandSyntax::Consume(_) => String::from("Some Text"),
            CommandSyntax::User(_, _) => String::from("123456789"),
            CommandSyntax::Member(_, _) => String::from("123456789"),
            CommandSyntax::String(_, _) => String::from("\"something\""),
            CommandSyntax::Duration(_, _) => String::from("15m"),
            CommandSyntax::Reason(_) => String::from("user broke a rule"),
            CommandSyntax::Number(_, _) => String::from("5"),
            CommandSyntax::Channel(_, _) => String::from("#some-channel"),
            CommandSyntax::Filters => String::from("+user @ouroboros"),
            CommandSyntax::Or(a, b) => format!("({} || {})", a.get_example(), b.get_example()),
        }
    }
}

/// A struct defining required permissions by both the user and the bot
#[derive(Default)]
pub struct CommandPermissions {
    /// Permissions which are required by the user to run the command
    pub required: Vec<Permissions>,
    /// Permissions where a user only needs to have one permission to run the command
    pub one_of: Vec<Permissions>,
    /// Permissions the bot needs to run the command
    pub bot: Vec<Permissions>,
}

impl CommandPermissions {
    /// Baseline permission set needed for bot operation
    pub fn baseline() -> Vec<Permissions> {
        vec![
            Permissions::VIEW_CHANNEL,
            Permissions::SEND_MESSAGES,
            Permissions::SEND_MESSAGES_IN_THREADS,
            Permissions::READ_MESSAGE_HISTORY,
            Permissions::ATTACH_FILES,
            Permissions::EMBED_LINKS,
            Permissions::ADD_REACTIONS,
        ]
    }

    /// Moderation permission set needed for bot moderation commands
    pub fn moderation() -> Vec<Permissions> {
        vec![
            Permissions::KICK_MEMBERS,
            Permissions::MODERATE_MEMBERS,
            Permissions::BAN_MEMBERS,
            Permissions::MANAGE_MESSAGES,
            Permissions::MANAGE_NICKNAMES,
            Permissions::MODERATE_MEMBERS,
        ]
    }
}

/// The base command trait which needs to be implemented by all command structs
#[async_trait]
pub trait Command: Send + Sync {
    // Command descriptors
    /// The name of the command, also used when actually running the command
    fn get_name(&self) -> &'static str;
    /// A short description of the command used in the help command list
    fn get_short(&self) -> &'static str;
    /// A long description of the command used in the individual command help
    fn get_full(&self) -> &'static str;
    /// The syntax of the command to display in the command help
    fn get_syntax(&self) -> Vec<CommandSyntax>;
    /// The category of the command to put it in the correct spot of the help command list
    fn get_category(&self) -> CommandCategory;

    // Runner
    /// The function to execute when running a command
    /// This function should also have the `#[command]` ouroboros macro attached
    /// The macro allows for easier definition of command arguments
    /// This can be left out if there are no arguments to define
    async fn run(
        &self,
        ctx: Context,
        msg: Message,
        _handler: &Handler,
        args: Vec<Token>,
        params: HashMap<&str, (bool, CommandArgument)>,
        _trace: &mut crate::utils::trace::TraceContext,
    ) -> Result<(), CommandError>;

    // Run helpers
    /// The transformers to use to process arguments
    /// This is normally generated by the `#[command]` ouroboros macro but can be left out if there are no arguments to define
    fn get_transformers(&self) -> Vec<TransformerFnArc> {
        vec![]
    }

    /// Returns the permissions required by the user/bot to run the command
    fn get_permissions(&self) -> CommandPermissions {
        CommandPermissions {
            bot: CommandPermissions::baseline(),
            ..Default::default()
        }
    }

    /// The optional position-less parameters the command can take
    fn get_params(&self) -> Vec<&'static CommandParameter<'static>>;
}

mod admin;
// pub use admin::Config;
pub use admin::CreateOcrRule;
pub use admin::DefineLog;
pub use admin::DeleteRule;
pub use admin::Encrypt;
pub use admin::OcrCheck;
pub use admin::Rules;

mod developer;
pub use developer::CacheSize;
pub use developer::ContextCmd;
pub use developer::Jeprof;
pub use developer::MsgDbg;
pub use developer::PermDbg;
pub use developer::Restart;
pub use developer::Say;
pub use developer::ScheduleDowntime;
pub use developer::Trace;
pub use developer::Update;

mod misc;
pub use misc::About;
pub use misc::ColonThree;
pub use misc::Ping;
pub use misc::Stats;

mod moderation;
pub use moderation::Ban;
pub use moderation::Duration;
pub use moderation::EditRef;
pub use moderation::Edits;
pub use moderation::Kick;
pub use moderation::Log;
pub use moderation::Mute;
pub use moderation::Purge;
pub use moderation::Reason;
pub use moderation::Ref;
pub use moderation::Softban;
pub use moderation::Unban;
pub use moderation::Unmute;
pub use moderation::Warn;

mod utilities;
pub use utilities::Cache;
pub use utilities::ExtractId;
