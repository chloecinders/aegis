import { block, decomment, inner, literal, pieces, rust } from "./rust.ts";

type Shape = "Positional" | "Optional" | "Rest" | "Reply" | "Flag";

export interface Modeled {
    name: string;
    inner: string;
    shape: Shape;
    short: string | null;
    desc: string;
}

export interface Parsed {
    fields: Modeled[];
    name: string;
    aliases: string[];
    short: string;
    full: string;
    category: string;
    user: string[];
    one_of: string[];
    developer: boolean;
    hidden: boolean;
}

export async function kinds(files: string[]): Promise<Map<string, string>> {
    const found = new Map<string, string>();
    const generators = new Map<string, string>();
    const bodies: string[] = [];

    const KIND = /const\s+KIND\s*:\s*ArgKind\s*=\s*ArgKind::([A-Za-z0-9_]+)\s*;/;

    for (const at of files) {
        const src = decomment(await rust(at));

        bodies.push(src);

        for (const match of src.matchAll(/macro_rules!\s+([A-Za-z0-9_]+)\s*\{/g)) {
            const body = inner(src, src.indexOf("{", match.index + match[0].length - 1));

            if (!/impl\s+FromArgs\s+for\s+\$ty\b/.test(body)) continue;

            const kind = KIND.exec(body);

            if (!kind) throw new Error(`${match[1]}! implements FromArgs without a KIND`);

            generators.set(match[1]!, kind[1]!);
        }

        for (const match of src.matchAll(/impl\s+FromArgs\s+for\s+([A-Za-z0-9_]+)\s*\{/g)) {
            const body = inner(src, src.indexOf("{", match.index + match[0].length - 1));
            const kind = KIND.exec(body);

            if (!kind) throw new Error(`FromArgs for ${match[1]} declares no KIND`);

            found.set(match[1]!, kind[1]!);
        }
    }

    for (const [generator, kind] of generators)
        for (const src of bodies)
            for (const match of src.matchAll(new RegExp(`(?:^|[^A-Za-z0-9_])${generator}!\\s*\\(\\s*([A-Za-z0-9_]+)\\s*\\)`, "g")))
                found.set(match[1]!, kind);

    return found;
}

export async function optionKinds(files: string[]): Promise<Map<string, string>> {
    const found = new Map<string, string>();

    const KIND = /const\s+KIND\s*:\s*CommandOptionType\s*=\s*CommandOptionType::([A-Za-z0-9_]+)\s*;/;

    for (const at of files) {
        const src = decomment(await rust(at));

        for (const match of src.matchAll(/impl\s+FromOption\s+for\s+([A-Za-z0-9_]+)\s*\{/g)) {
            const body = inner(src, src.indexOf("{", match.index + match[0].length - 1));
            const kind = KIND.exec(body);

            if (!kind) throw new Error(`FromOption for ${match[1]} declares no KIND`);

            found.set(match[1]!, kind[1]!);
        }
    }

    return found;
}

function fields(body: string, at: string): Modeled[] {
    const found: Modeled[] = [];

    let attributes: { flag: boolean; options: string[] }[] = [];
    let start = 0;
    let cursor = 0;

    const peel = (ty: string, wrapper: string): string | null => {
        const opened = ty.indexOf("<");

        if (opened < 0 || ty.slice(0, opened).trim() !== wrapper) return null;

        return ty.slice(opened + 1, ty.lastIndexOf(">")).trim();
    };

    while (cursor < body.length) {
        if (body[cursor] === "#") {
            const opened = body.indexOf("[", cursor);
            const text = inner(body, opened);
            const head = /^\s*([A-Za-z0-9_]+)/.exec(text);

            if (head && (head[1] === "arg" || head[1] === "flag")) {
                const opens = text.indexOf("(");

                attributes.push({
                    flag: head[1] === "flag",
                    options: opens < 0 ? [] : pieces(inner(text, opens), ","),
                });
            }

            cursor = block(body, opened);
            start = cursor;

            continue;
        }

        if (body[cursor] !== ",") {
            cursor = "{[(<".includes(body[cursor]!) ? block(body, cursor) : cursor + 1;

            continue;
        }

        const declaration = body.slice(start, cursor).trim();

        cursor += 1;
        start = cursor;

        if (!declaration) continue;

        const split = declaration.indexOf(":");
        const name = declaration
            .slice(0, split)
            .replace(/\bpub(\s*\([^)]*\))?\b/, "")
            .trim();
        const declared = declaration.slice(split + 1).trim();

        if (attributes.length !== 1) throw new Error(`${at}.${name} needs exactly one #[arg] or #[flag]`);

        const { flag, options } = attributes[0]!;

        attributes = [];

        let rest = false;
        let reply = false;
        let called = name;
        let short: string | null = null;
        let desc = "";

        for (const option of options) {
            if (option === "rest") rest = true;
            else if (option === "reply") reply = true;
            else if (/^amend\s*=/.test(option)) continue;
            else if (/^name\s*=/.test(option)) called = literal(option, option.indexOf('"')).value;
            else if (/^short\s*=/.test(option)) {
                const letter = /^short\s*=\s*'(\\.|[^'])'$/.exec(option);

                if (!letter) throw new Error(`${at}.${name} has an unreadable short ${option}`);

                short = letter[1]!.replace("\\", "");
            } else if (/^desc\s*=/.test(option)) desc = literal(option, option.indexOf('"')).value;
            else throw new Error(`${at}.${name} carries an unknown option ${option}`);
        }

        const optional = peel(declared, "Option");
        const carrier = optional ?? declared;
        const wrapped = peel(carrier, "Arg");

        if (reply && optional) throw new Error(`${at}.${name} is an optional reply, which args.rs has no shape for`);

        const shape: Shape = flag ? "Flag" : rest ? "Rest" : reply ? "Reply" : optional ? "Optional" : "Positional";

        found.push({ name: called, inner: wrapped ?? carrier, shape, short, desc });
    }

    if (body.slice(start).trim() || attributes.length)
        throw new Error(`${at} ends in a field with no trailing comma, which the sheet cannot read`);

    return found;
}

function metaBlock(body: string, at: string): Omit<Parsed, "fields"> {
    const entries = new Map<string, string>();

    for (const piece of pieces(body, ",")) {
        const split = piece.indexOf(":");

        if (split < 0) throw new Error(`${at} has a meta entry with no key: ${piece}`);

        entries.set(piece.slice(0, split).trim(), piece.slice(split + 1).trim());
    }

    for (const key of entries.keys())
        if (!["name", "aliases", "short", "full", "category", "user", "one_of", "bot", "developer", "hidden", "edit"].includes(key))
            throw new Error(`${at} sets an unknown meta key ${key}`);
    for (const key of ["name", "short", "full", "category"])
        if (!entries.has(key)) throw new Error(`${at} is missing meta \`${key}\``);

    const text = (key: string): string => literal(entries.get(key)!, 0).value;
    const list = (key: string): string[] => {
        const raw = entries.get(key);

        return raw ? pieces(raw.slice(1, -1), ",") : [];
    };

    return {
        name: text("name"),
        aliases: list("aliases").map((one) => literal(one, 0).value),
        short: text("short"),
        full: text("full"),
        category: entries.get("category")!,
        user: list("user"),
        one_of: list("one_of"),
        developer: entries.get("developer") === "true",
        hidden: entries.get("hidden") === "true",
    };
}

async function structs(files: string[], attribute: string, implemented: string): Promise<Map<string, Parsed>> {
    const found = new Map<string, Parsed>();
    const declared = new RegExp(`#\\[${attribute}\\]\\s*(?:pub(?:\\s*\\([^)]*\\))?\\s+)?struct\\s+([A-Za-z0-9_]+)\\s*([{;])`, "g");

    for (const at of files) {
        const src = decomment(await rust(at));

        for (const match of src.matchAll(declared)) {
            const struct = match[1]!;
            const impl = new RegExp(`impl\\s+${implemented}\\s+for\\s+${struct}\\s*\\{`).exec(src);

            if (!impl) throw new Error(`${struct} is a #[${attribute}] struct with no ${implemented} impl`);

            const body = inner(src, src.indexOf("{", impl.index + impl[0].length - 1));
            const meta = /const\s+META\s*:\s*Meta\s*=\s*meta!\s*\{/.exec(body);

            if (!meta) throw new Error(`${struct} implements ${implemented} without a meta! block`);

            if (found.has(struct)) throw new Error(`two #[${attribute}] structs are named ${struct}`);

            found.set(struct, {
                fields: match[2] === ";" ? [] : fields(inner(src, match.index + match[0].length - 1), struct),
                ...metaBlock(inner(body, body.indexOf("{", meta.index + meta[0].length - 1)), struct),
            });
        }
    }

    return found;
}

export function commands(files: string[]): Promise<Map<string, Parsed>> {
    return structs(files, "command", "Command");
}

export function slashes(files: string[]): Promise<Map<string, Parsed>> {
    return structs(files, "slash", "Slash");
}
