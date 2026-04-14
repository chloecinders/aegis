use serenity::all::{CacheHttp, Context, Guild, GuildId, GuildInfo, GuildPagination, PartialGuild};

pub enum CachedGuild {
    Cached(Guild),
    Partial(PartialGuild),
}

impl CachedGuild {
    pub fn name(&self) -> String {
        match self {
            CachedGuild::Cached(g) => g.name.clone(),
            CachedGuild::Partial(p) => p.name.clone(),
        }
    }
}

pub async fn get_guild_info(ctx: &Context, guild_id: Option<GuildId>) -> Option<CachedGuild> {
    let id = guild_id?;

    if let Some(g) = ctx.cache.guild(id) {
        return Some(CachedGuild::Cached(g.clone()));
    }

    if let Ok(p) = id.to_partial_guild(ctx).await {
        return Some(CachedGuild::Partial(p));
    }

    None
}

pub async fn get_all_guilds(http: impl CacheHttp) -> Vec<GuildInfo> {
    let mut result: Vec<GuildInfo> = Vec::new();

    loop {
        let last_page = result.last().map(|g| GuildPagination::After(g.id));
        let guilds = http.http().get_guilds(last_page, None).await;

        if let Ok(guilds) = guilds {
            if guilds.len() == 0 {
                break;
            }

            for guild in guilds {
                result.push(guild);
            }
        } else {
            break;
        }
    }

    result
}
