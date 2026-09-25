import { PERMISSIONS } from "./grammar.ts";

export interface Token {
    text: string;
    at: number;
    kind: "str" | "rgx" | "word";
}

export type Kind = "literal" | "regex" | "wildcard";

export interface Matcher {
    kind: Kind;
    text: string;
}

export type Read = (Matcher & { error?: never }) | { kind?: never; text?: never; error: string };

export function tokenise(line: string): Token[] {
    const out: Token[] = [];
    let i = 0;

    while (i < line.length) {
        const ch = line[i];

        if (/\s/.test(ch)) {
            i++;
            continue;
        }

        if (ch === '"') {
            let j = i + 1;

            while (j < line.length && line[j] !== '"') j += line[j] === "\\" ? 2 : 1;

            out.push({ text: line.slice(i, Math.min(j + 1, line.length)), at: i, kind: "str" });
            i = j + 1;
            continue;
        }

        if (ch === "/") {
            let j = i + 1;

            while (j < line.length && line[j] !== "/") j += line[j] === "\\" ? 2 : 1;

            out.push({ text: line.slice(i, Math.min(j + 1, line.length)), at: i, kind: "rgx" });
            i = j + 1;
            continue;
        }

        let j = i;

        while (j < line.length && !/\s/.test(line[j])) {
            const close = line[j] === "<" ? line.indexOf(">", j) : -1;

            j = close === -1 ? j + 1 : close + 1;
        }

        out.push({ text: line.slice(i, j), at: i, kind: "word" });
        i = j;
    }

    return out;
}

export function readMatcher(rest: string): Read {
    const trimmed = rest.trim();

    if (!trimmed) return { error: "empty pattern" };

    const opener = trimmed[0];
    const quoted = (opener === '"' || opener === "'") && trimmed.length >= 2 && trimmed.endsWith(opener);
    const body = quoted ? trimmed.slice(1, -1) : trimmed;

    if (body.length >= 2 && body.startsWith("/") && body.endsWith("/")) {
        const pattern = body.slice(1, -1);

        try {
            new RegExp(pattern);
        } catch {
            return { error: "regex does not compile" };
        }

        return { kind: "regex", text: pattern };
    }

    if (body.length >= 2 && body.startsWith("|") && body.endsWith("|")) {
        const pattern = body.slice(1, -1);

        if (!pattern) return { error: "empty pattern" };

        return { kind: "wildcard", text: pattern };
    }

    if (!body) return { error: "empty pattern" };

    return { kind: "literal", text: body };
}

export function idOf(tok: Token, prefix: string): string | null {
    const raw = tok.text;
    const tagged = prefix + ":";

    if (raw.startsWith(tagged)) {
        const id = raw.slice(tagged.length);

        return /^\d{4,20}$/.test(id) ? id : null;
    }

    const inner = raw.startsWith("<") && raw.endsWith(">") ? raw.slice(1, -1) : null;

    if (inner) {
        const mark = prefix === "role" ? "@&" : "#";

        if (inner.startsWith(mark)) {
            const id = inner.slice(mark.length);

            return /^\d{4,20}$/.test(id) ? id : null;
        }
    }

    return null;
}

export function permissionOf(tok: Token): string | null {
    const raw = tok.text.toLowerCase();

    if (!raw.startsWith("permission:")) return null;

    const name = raw.slice("permission:".length).replace(/-/g, "_");

    return PERMISSIONS.includes(name) ? name : null;
}

export function bareChannel(tok: Token): string | null {
    return /^\d{4,20}$/.test(tok.text) ? tok.text : null;
}
