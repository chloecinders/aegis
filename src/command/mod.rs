pub mod amend;
pub mod args;
pub mod caller;
pub mod cx;
pub mod edit;
pub mod error;
pub mod flags;
pub mod help;
pub mod permissions;
pub mod pipeline;
pub mod registry;
pub mod retract;
pub mod slash;
pub mod stream;
pub mod typing;
pub mod value;

use std::fmt::{self, Display};
use std::future::Future;
use std::pin::Pin;

use serenity::all::{MessageId, Permissions};

use crate::command::args::Field;
use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::stream::Stream;
use crate::platform::ui::embed::Embed;

pub type Boxed<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Args: Sized + Send + 'static {
    const FIELDS: &'static [Field];

    fn parse(cx: &Cx, stream: &mut Stream) -> impl Future<Output = Result<Self>> + Send;

    fn snapshot(&self) -> serde_json::Value;
}

pub trait Command: Args {
    const META: Meta;

    fn run(self, cx: &mut Cx) -> impl Future<Output = Result<Response>> + Send;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Misc,
    Utilities,
    Moderation,
    Records,
    Admin,
    Developer,
}

pub const CATEGORIES: [Category; 6] = [
    Category::Misc,
    Category::Utilities,
    Category::Moderation,
    Category::Records,
    Category::Admin,
    Category::Developer,
];

impl Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Category::Misc => "Misc",
            Category::Utilities => "Utilities",
            Category::Moderation => "Moderation",
            Category::Records => "Records",
            Category::Admin => "Admin",
            Category::Developer => "Developer",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditMode {
    Rerun,
    Amendable,
    Fixed,
}

pub const BASELINE: Permissions = Permissions::from_bits_truncate(
    Permissions::VIEW_CHANNEL.bits()
        | Permissions::SEND_MESSAGES.bits()
        | Permissions::SEND_MESSAGES_IN_THREADS.bits()
        | Permissions::READ_MESSAGE_HISTORY.bits()
        | Permissions::ATTACH_FILES.bits()
        | Permissions::EMBED_LINKS.bits()
        | Permissions::ADD_REACTIONS.bits(),
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub short: &'static str,
    pub full: &'static str,
    pub category: Category,
    pub user: Permissions,
    pub one_of: Permissions,
    pub bot: Permissions,
    pub developer: bool,
    pub hidden: bool,
    pub edit: EditMode,
}

#[derive(Debug)]
pub enum Response {
    Embed(Box<Embed>),
    None,
    Sent(MessageId),
}

impl Response {
    pub fn embed(embed: Embed) -> Self {
        Response::Embed(Box::new(embed))
    }
}
