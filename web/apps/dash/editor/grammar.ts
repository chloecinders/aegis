export interface Stub {
    keyword: string;
    takes: string;
    stub: string;
}

export interface Clause {
    keyword: string;
    takes: string;
    about: string;
    values: string[][];
}

export const MAX_PATTERNS = 64;

export const FUZZ = 0.95;

export const SOURCES = ["content", "image", "filename", "mimetype", "embed", "username", "join"];
export const MEASURES = ["mentions", "links", "invites", "attachments"];
export const RECORD = ["warns", "mutes", "kicks", "bans", "punishments"];

export const PERMISSIONS = [
    "create_instant_invite",
    "kick_members",
    "ban_members",
    "administrator",
    "manage_channels",
    "manage_guild",
    "add_reactions",
    "view_audit_log",
    "priority_speaker",
    "stream",
    "view_channel",
    "send_messages",
    "send_tts_messages",
    "manage_messages",
    "embed_links",
    "attach_files",
    "read_message_history",
    "mention_everyone",
    "use_external_emojis",
    "view_guild_insights",
    "connect",
    "speak",
    "mute_members",
    "deafen_members",
    "move_members",
    "use_vad",
    "change_nickname",
    "manage_nicknames",
    "manage_roles",
    "manage_webhooks",
    "manage_guild_expressions",
    "manage_emojis_and_stickers",
    "use_application_commands",
    "request_to_speak",
    "manage_events",
    "manage_threads",
    "create_public_threads",
    "create_private_threads",
    "use_external_stickers",
    "send_messages_in_threads",
    "use_embedded_activities",
    "moderate_members",
    "view_creator_monetization_analytics",
    "use_soundboard",
    "create_guild_expressions",
    "create_events",
    "use_external_sounds",
    "send_voice_messages",
    "set_voice_channel_status",
    "send_polls",
    "use_external_apps",
];

export const RETIRED: Record<string, string> = { punishments: "punishments" };
export const KEPT = ["warns", "mutes", "kicks", "bans"];
export const CMPS = [">=", "<=", ">", "<"];
export const VERBS = ["warn", "kick", "ban", "softban", "mute", "delete"];
export const PUNISHMENTS = ["warn", "kick", "ban", "softban", "mute"];

export const TEMPLATE = ["on content", "", 'match "something"', "", "then delete"].join("\n");

export const CATCHES = ["on content", "", 'match "something"'].join("\n");

export const RESPONSE: Stub[] = [
    { keyword: "then", takes: "warn | mute 10m | kick | ban 7d | softban | delete", stub: "then " },
    { keyword: "delete", takes: "nothing", stub: "delete" },
    { keyword: "clear", takes: "0-7", stub: "clear " },
    { keyword: "reason", takes: "text", stub: "reason " },
    { keyword: "after", takes: "2 in 10m", stub: "after " },
    { keyword: "notify", takes: "channel:<id> | none", stub: "notify channel:" },
    { keyword: "ignore", takes: "role:<id> | channel:<id> | permission:<name>", stub: "ignore " },
    { keyword: "only", takes: "channel:<id>", stub: "only channel:" },
];

export const CLAUSES: Clause[] = [
    {
        keyword: "on",
        takes: "image | content | filename | mimetype | embed | username | join",
        about: "Which source the rule reads from.",
        values: [
            ["content", "message content"],
            ["image", "image text content"],
            ["filename", "attached file names"],
            ["mimetype", "attached file MIME types"],
            ["embed", "text inside of embeds"],
            ["username", "member usernames"],
            ["join", "member join, can not be used with match"],
        ],
    },
    {
        keyword: "match",
        takes: '"text" | /regex/',
        about: "The matches of a rule. Writing multiple matches acts as OR and will match either one of them.",
        values: [
            ['"text"', "a text literal, matched loosely to prevent bad OCR reads from not matching"],
            ["/regex/", "a regular expression, matched exactly as written (uses the Rust regex engine)"],
        ],
    },
    {
        keyword: "never",
        takes: '"text" | /regex/',
        about: "An exception to match. If something matches a match, but also matches a never, the rule won't trigger.",
        values: [
            ['"text"', "a text literal, matched loosely to prevent bad OCR reads from not matching"],
            ["/regex/", "a regular expression, matched exactly as written (uses the Rust regex engine)"],
        ],
    },
    {
        keyword: "when",
        takes: "mentions | links | invites | attachments > n | warns > n in 30d | account younger than 7d",
        about: "A condition on the source. Allows for checking specific things about the source before matching.",
        values: [
            ["mentions", "how many people the message pings"],
            ["links", "how many links the message contains"],
            ["invites", "how many Discord invites the message contains"],
            ["attachments", "how many files are attached"],
            ["account", "the age of an account (written as 'younger than' or 'older than')"],
            ["warns", "warnings on their log"],
            ["mutes", "mutes on their log"],
            ["kicks", "kicks on their log"],
            ["bans", "bans on their log"],
            ["punishments", "everything on their log"],
        ],
    },
    {
        keyword: "only",
        takes: "channel:<id>",
        about: "Limits a rule to one or more specific channels.",
        values: [["channel:<id>", "a specified channel"]],
    },
    {
        keyword: "ignore",
        takes: "role:<id> | channel:<id> | permission:<name>",
        about: "Specifies when the rule is not applied, such as for members with specific roles or permissions or in entire channels.",
        values: [
            ["role:<id>", "a role"],
            ["channel:<id>", "a channel"],
            ["permission:<name>", "a global Discord server permission"],
        ],
    },
    {
        keyword: "after",
        takes: "2 in 10m",
        about: "Makes the rule only act after a specific amount of violations within a timeframe.",
        values: [
            ["2", "how many matches it takes, two or more"],
            ["10m", "the timeframe"],
        ],
    },
    {
        keyword: "then",
        takes: "warn | kick | ban [7d] | softban | mute [10m] | delete",
        about: "The action that should be taken when the rule gets triggered. All actions except delete additionally result in a log.",
        values: [
            ["warn", "applies a warning to the member"],
            ["kick", "kicks the member out of the server"],
            ["ban [7d]", "bans the member from the server for the specified time"],
            ["softban", "softbans the member from the server"],
            ["mute [10m]", "times the member out for the specified time"],
            ["delete", "removes the original message"],
        ],
    },
    {
        keyword: "delete",
        takes: "",
        about: "Additionally removes the original message combined with the 'then' punishment.",
        values: [],
    },
    {
        keyword: "clear",
        takes: "0-7",
        about: "How many days of messages a ban/softban clears.",
        values: [
            ["1", "one day"],
            ["7", "upper limit"],
        ],
    },
    {
        keyword: "notify",
        takes: "channel:<id> | none",
        about: "Posts triggers in a specified channel instead of the moderation log, or nowhere at all with none.",
        values: [
            ["channel:<id>", "the notice channel"],
            ["none", "post nothing"],
        ],
    },
    {
        keyword: "reason",
        takes: "text",
        about: "The reason on any punishment.",
        values: [],
    },
];

export const KEYWORDS = CLAUSES.map((clause) => clause.keyword);
