pub struct Clause {
    pub keyword: &'static str,
    pub short: &'static str,
    pub full: &'static str,
    pub params: &'static [(&'static str, &'static str)],
    pub examples: &'static [&'static str],
}

pub const CLAUSES: [Clause; 12] = [
    Clause {
        keyword: "on",
        short: "image | content | filename | mimetype | embed | username | join",
        full: "Which source the rule reads from.",
        params: &[
            ("content", "message content"),
            ("image", "image text content"),
            ("filename", "attached file names"),
            ("mimetype", "attached file MIME types"),
            ("embed", "text inside of embeds"),
            ("username", "member usernames"),
            ("join", "member join, can not be used with match"),
        ],
        examples: &["on content", "on image username filename", "on join"],
    },
    Clause {
        keyword: "match",
        short: "\"text\" | /regex/",
        full: "The matches of a rule. Writing multiple matches acts as OR and will match either one of them.",
        params: &[
            (
                "\"text\"",
                "a text literal, matched loosely to prevent bad OCR reads from not matching",
            ),
            (
                "/regex/",
                "a regular expression, matched exactly as written (uses the Rust regex engine)",
            ),
        ],
        examples: &["match \"free nitro\"", "match /([A-Z])\\w+/"],
    },
    Clause {
        keyword: "never",
        short: "\"text\" | /regex/",
        full: "An exception to match. If something matches a match, but also matches a never, the rule won't trigger.",
        params: &[
            (
                "\"text\"",
                "a text literal, matched loosely to prevent bad OCR reads from not matching",
            ),
            (
                "/regex/",
                "a regular expression, matched exactly as written (uses the Rust regex engine)",
            ),
        ],
        examples: &["never \"nitro giveaway rules\""],
    },
    Clause {
        keyword: "when",
        short: "mentions | links | invites | attachments > n | warns > n in 30d | account younger than 7d",
        full: "A condition on the source. Allows for checking specific things about the source before matching.",
        params: &[
            ("mentions", "how many people the message pings"),
            ("links", "how many links the message contains"),
            ("invites", "how many Discord invites the message contains"),
            ("attachments", "how many files are attached"),
            (
                "account",
                "the age of an account (written as 'younger than' or 'older than')",
            ),
            ("warns", "warnings on their log"),
            ("mutes", "mutes on their log"),
            ("kicks", "kicks on their log"),
            ("bans", "bans on their log"),
            ("punishments", "everything on their log"),
        ],
        examples: &[
            "when mentions > 5",
            "when account younger than 7d",
            "when warns >= 2 in 30d",
            "when punishments > 0",
        ],
    },
    Clause {
        keyword: "only",
        short: "channel:<id>",
        full: "Limits a rule to one or more specific channels.",
        params: &[("channel:<id>", "a specified channel")],
        examples: &[
            "only channel:112233445566778899",
            "only channel:112233445566778899 channel:998877665544332211",
        ],
    },
    Clause {
        keyword: "ignore",
        short: "role:<id> | channel:<id> | permission:<name>",
        full: "Specifies when the rule is not applied, such as for members with specific roles or permissions or in entire channels.",
        params: &[
            ("role:<id>", "a role"),
            ("channel:<id>", "a channel"),
            ("permission:<name>", "a global Discord server permission"),
        ],
        examples: &[
            "ignore role:112233445566778899 channel:998877665544332211",
            "ignore permission:manage_messages",
        ],
    },
    Clause {
        keyword: "after",
        short: "2 in 10m",
        full: "Makes the rule only act after a specific amount of violations within a timeframe.",
        params: &[
            ("2", "how many matches it takes, two or more"),
            ("10m", "the timeframe"),
        ],
        examples: &["after 3 in 5m"],
    },
    Clause {
        keyword: "then",
        short: "warn | kick | ban [7d] | softban | mute [10m] | delete",
        full: "The action that should be taken when the rule gets triggered. All actions except delete additionally result in a log.",
        params: &[
            ("warn", "applies a warning to the member"),
            ("kick", "kicks the member out of the server"),
            (
                "ban [7d]",
                "bans the member from the server for the specified time",
            ),
            ("softban", "softbans the member from the server"),
            ("mute [10m]", "times the member out for the specified time"),
            ("delete", "removes the original message"),
        ],
        examples: &["then ban 7d", "then kick", "then mute 10m", "then delete"],
    },
    Clause {
        keyword: "delete",
        short: "",
        full: "Additionally removes the original message combined with the 'then' punishment.",
        params: &[],
        examples: &["delete"],
    },
    Clause {
        keyword: "clear",
        short: "0-7",
        full: "How many days of messages a ban/softban clears.",
        params: &[("1", "one day"), ("7", "upper limit")],
        examples: &["clear 1"],
    },
    Clause {
        keyword: "notify",
        short: "channel:<id> | none",
        full: "Posts triggers in a specified channel instead of the moderation log, or nowhere at all with none.",
        params: &[
            ("channel:<id>", "the notice channel"),
            ("none", "post nothing"),
        ],
        examples: &["notify channel:112233445566778899", "notify none"],
    },
    Clause {
        keyword: "reason",
        short: "text",
        full: "The reason on any punishment.",
        params: &[],
        examples: &["reason scam bot"],
    },
];
