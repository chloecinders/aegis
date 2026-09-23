import { readPattern, readSources, readWhen } from "./clauses/detection.ts";
import { readAction, readClear, readDelete, readNotify, readReason, readThreshold } from "./clauses/response.ts";
import { readScope } from "./clauses/scope.ts";
import { KEYWORDS, MAX_PATTERNS, PUNISHMENTS, RECORD } from "./grammar.ts";
import type { Body, Clause, Diag, Parsed, Part } from "./rule.ts";
import { tokenise } from "./tokenise.ts";

const DETECTION = ["on", "match", "never", "when"];

const READERS: Record<string, (clause: Clause) => void> = {
    on: readSources,
    match: readPattern,
    never: readPattern,
    when: readWhen,
    only: readScope,
    ignore: readScope,
    after: readThreshold,
    then: readAction,
    delete: readDelete,
    clear: readClear,
    notify: readNotify,
    reason: readReason,
};

export function parse(source: string, part: Part = "whole"): Parsed {
    const body: Body = {
        sources: [],
        match: [],
        never: [],
        when: [],
        only: [],
        ignoreRoles: [],
        ignoreChannels: [],
        ignorePermissions: [],
        after: null,
        then: null,
        delete: false,
        clear: null,
        notify: null,
        reason: null,
    };

    const diags: Diag[] = [];
    const lines = source.split("\n");
    const seen: Record<string, number> = {};

    lines.forEach((raw, index) => {
        const line = index + 1;
        const text = raw.trim();

        if (text === "") return;

        const tokens = tokenise(text);
        const head = tokens[0];
        const word = head.text.toLowerCase();

        const bad = (msg: string, help?: string, fill?: string) => diags.push({ line, level: "error", msg, help, fill });
        const warn = (msg: string) => diags.push({ line, level: "warn", msg });

        if (!KEYWORDS.includes(word)) {
            bad("no clause found");
            return;
        }

        if (part === "detection" && !DETECTION.includes(word)) {
            bad("this block only takes detection clauses");
            return;
        }

        if (part === "response" && DETECTION.includes(word)) {
            bad("this block only takes response clauses");
            return;
        }

        READERS[word]({
            body,
            seen,
            line,
            text,
            head,
            word,
            rest: tokens.slice(1),
            bad,
            warn,
            once: (msg: string) => {
                if (seen[word]) {
                    bad(msg);
                    return false;
                }

                seen[word] = line;

                return true;
            },
        });
    });

    if (part === "response") return { body, diags, errors: diags.filter((diag) => diag.level === "error").length };

    const patterns = body.match.length + body.never.length;

    if (patterns > MAX_PATTERNS) diags.push({ line: 0, level: "error", msg: "too many patterns" });

    const joining = body.sources.length > 0 && body.sources.every((source) => source === "join");
    const textless =
        body.sources.length > 0 && body.sources.every((source) => source === "join" || source === "message");

    if (textless && body.match.length)
        diags.push({ line: 0, level: "error", msg: "source has no text" });

    const grounded = body.when.find((when) => when.measure !== "account" && !RECORD.includes(when.measure));

    if (joining && grounded)
        diags.push({ line: grounded.line, level: "error", msg: "measure not available on this source" });

    if (joining && body.then && PUNISHMENTS.includes(body.then.verb))
        diags.push({ line: 0, level: "warn", msg: "join rules never act" });

    if (!body.match.length && !body.when.length)
        diags.push({ line: 0, level: "error", msg: "missing match or when clause" });

    if (body.clear !== null && (!body.then || body.then.verb !== "ban"))
        diags.push({ line: 0, level: "warn", msg: "clear only applies to a ban" });

    if (!body.sources.length) body.sources = ["content", "image", "filename", "embed"];

    return { body, diags, errors: diags.filter((diag) => diag.level === "error").length };
}
