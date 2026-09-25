pub mod archive;
pub mod automod;
pub mod diagnostics;
pub mod errorlog;
pub mod guildlog;
pub mod help;
pub mod info;
pub mod permissions;
pub mod punishments;
pub mod records;
pub mod references;
pub mod settings;
pub mod snippets;
pub mod sticky;

use crate::command::registry::Registry;
use crate::platform::discord::dispatch::Dispatch;
use crate::platform::discord::interact::Router;

pub fn observe(dispatch: &mut Dispatch) {
    archive::observe(dispatch);
    automod::observe(dispatch);
    guildlog::observe(dispatch);
    punishments::observe(dispatch);
    sticky::observe(dispatch);
}

pub fn control(router: &mut Router) {
    archive::control(router);
    automod::control(router);
    help::control(router);
    records::control(router);
    references::control(router);
    settings::control(router);
}

pub fn register(registry: &mut Registry) {
    archive::register(registry);
    automod::register(registry);
    diagnostics::register(registry);
    help::register(registry);
    info::register(registry);
    permissions::register(registry);
    records::register(registry);
    references::register(registry);
    settings::register(registry);
    snippets::register(registry);
    sticky::register(registry);
    punishments::register(registry);
}
