export interface Flag {
    name: string;
    switch: string;
    desc: string;
}

export interface SlashOption {
    name: string;
    kind: string;
    desc: string;
    required: boolean;
}

export interface Documented {
    name: string;
    aliases: string[];
    short: string;
    full: string;
    category: string;
    developer: boolean;
    hidden: boolean;
    syntax: string | null;
    example: string | null;
    user: string[];
    one_of: string[];
    flags: Flag[];
    slash: SlashOption[] | null;
}

export interface Sheet {
    categories: string[];
    commands: Documented[];
}

export interface Grouping {
    name: string;
    commands: Documented[];
}

export interface Token {
    kind: string;
    text: string;
}

export const safeName = (name: string) => name.replace(/[^a-z0-9]/gi, "_");

export function titleCase(str: string): string {
    return str
        .replace(/^_/, "")
        .split(/[_-]/)
        .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
        .join(" ");
}

export function normalise(text: string): string {
    if (!text) return "";

    return text
        .replaceAll("/p/", "+")
        .replace(/\s\s+/g, " ")
        .replace(/\*\*(.*?)\*\*/g, "<strong>$1</strong>")
        .replace(/\*(.*?)\*/g, "<em>$1</em>")
        .replace(/`(.*?)`/g, "<code>$1</code>")
        .trim();
}

function emit(out: Token[], kind: string, text: string) {
    const last = out[out.length - 1];

    if (last && last.kind === kind) last.text += text;
    else out.push({ kind, text });
}

const COMMAND = /^[+/]\S+/;

export function highlightSyntax(line: string): Token[] {
    const out: Token[] = [];

    let rest = line;
    let typing = false;

    while (rest.length) {
        let take = (kind: string, text: string) => {
            emit(out, kind, text);
            rest = rest.slice(text.length);
        };

        let match;

        if (!out.length && (match = COMMAND.exec(rest))) take("cmd", match[0]);
        else if ((match = /^\s+/.exec(rest))) take("plain", match[0]);
        else if ((match = /^\.\.\./.exec(rest))) take("punct", match[0]);
        else if ((match = /^\|\|/.exec(rest))) (typing = false), take("op", match[0]);
        else if ((match = /^[<[(>\])]/.exec(rest))) (typing = false), take("punct", match[0]);
        else if ((match = /^:/.exec(rest))) (typing = true), take("punct", match[0]);
        else if (typing && (match = /^[A-Za-z_][\w]*(?: [A-Za-z][\w]*)*/.exec(rest))) take("type", match[0]);
        else if ((match = /^[A-Za-z_][\w]*/.exec(rest))) take(match[0] === "reply" ? "keyword" : "param", match[0]);
        else take("plain", rest[0]);
    }

    return out;
}

export function highlightExample(line: string): Token[] {
    const out: Token[] = [];

    let rest = line;

    while (rest.length) {
        let take = (kind: string, text: string) => {
            emit(out, kind, text);
            rest = rest.slice(text.length);
        };

        let match;

        if (!out.length && (match = COMMAND.exec(rest))) take("cmd", match[0]);
        else if ((match = /^\s+/.exec(rest))) take("plain", match[0]);
        else if ((match = /^"[^"]*"/.exec(rest))) take("str", match[0]);
        else if ((match = /^https?:\/\/\S+/.exec(rest))) take("url", match[0]);
        else if ((match = /^[@#]\S+/.exec(rest))) take("mention", match[0]);
        else if ((match = /^\+\w+/.exec(rest))) take("flag", match[0]);
        else if ((match = /^\d+[smhdwy]\b/.exec(rest))) take("duration", match[0]);
        else if ((match = /^\d+\b/.exec(rest))) take("num", match[0]);
        else if ((match = /^(?=\w*\d)(?=\w*[A-Za-z])\w{4,}\b/.exec(rest))) take("id", match[0]);
        else if ((match = /^\w+/.exec(rest))) take("plain", match[0]);
        else take("plain", rest[0]);
    }

    return out;
}

export function grouped(sheet: Sheet): { commands: Documented[]; categories: Grouping[] } {
    const commands = sheet.commands
        .filter((cmd) => !cmd.hidden)
        .map((cmd) => ({
            ...cmd,
            short: normalise(cmd.short),
            full: normalise(cmd.full),
            flags: cmd.flags.map((flag) => ({ ...flag, desc: normalise(flag.desc) })),
            slash: cmd.slash?.map((option) => ({ ...option, desc: normalise(option.desc) })) ?? null,
        }));

    const stray = commands.filter((cmd) => !sheet.categories.includes(cmd.category));

    if (stray.length > 0) throw new Error(`unknown categor(ies) in commands.json: ${stray.map((c) => c.category).join(", ")}`);

    const categories = sheet.categories
        .map((category) => ({
            name: category,
            commands: commands.filter((cmd) => cmd.category === category).sort((a, b) => a.name.localeCompare(b.name)),
        }))
        .filter((category) => category.commands.length > 0);

    return { commands, categories };
}
