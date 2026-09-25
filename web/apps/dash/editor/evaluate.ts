import { showDuration } from "./duration.ts";
import { FUZZ, KEPT, RECORD } from "./grammar.ts";
import type { Body, When } from "./rule.ts";
import type { Matcher } from "./tokenise.ts";

export interface Step {
    state: "hit" | "miss" | "stop";
    what: string;
    val: string;
}

export interface Seen {
    source: string;
    text: string;
    channel: string;
    roles: string[];
    permissions: string[];
    age: number;
    mentions: number;
    links: number;
    invites: number;
    attachments: number;
    animated: number;
    record: Record<string, string>;
}

export interface Count {
    trace: Step[];
    matched: boolean;
    stopped: string | null;
    cleared: boolean;
    caught: boolean;
    satisfied: boolean;
}

function near(needle: string, haystack: string, threshold: number): boolean {
    if (haystack.includes(needle)) return true;

    const want = needle.toLowerCase();
    const lowered = haystack.toLowerCase();

    if (lowered.includes(want)) return true;

    const pattern = [...want];
    const text = [...lowered];

    if (pattern.length === 0) return false;

    const allowed = Math.ceil((1 - threshold) * pattern.length);

    let previous = new Array(pattern.length + 1);

    for (let at = 0; at <= pattern.length; at++) previous[at] = at;

    let best = previous[pattern.length];

    for (let row = 1; row <= text.length; row++) {
        const current = new Array(pattern.length + 1);

        current[0] = 0;

        for (let at = 1; at <= pattern.length; at++) {
            const cost = pattern[at - 1] === text[row - 1] ? 0 : 1;

            current[at] = Math.min(previous[at] + 1, current[at - 1] + 1, previous[at - 1] + cost);
        }

        best = Math.min(best, current[pattern.length]);
        previous = current;
    }

    return best <= allowed;
}

const tight = (text: string) => text.replace(/\s+/g, "");

function loosely(needle: string, haystack: string, threshold: number): boolean {
    return near(needle, haystack, threshold) || near(tight(needle), tight(haystack), threshold);
}

export const punishmentCount = (record: Record<string, string>, measure: string): number =>
    measure === "punishments"
        ? KEPT.reduce((total, kind) => total + Number(record[kind] || 0), 0)
        : Number(record[measure] || 0);

export const LINKS = /\b(?:https?:\/\/|www\.)\S+|\b[a-z0-9-]+\.(?:com|net|org|gg|gift|io|xyz|ru|cn)\b\S*/gi;
export const INVITES = /(?:discord\.gg|discord(?:app)?\.com\/invite)\/[a-z0-9-]+/gi;
export const MENTIONS = /<@!?\d+>|@everyone|@here/g;
export const count = (text: string, pattern: RegExp) => (text.match(pattern) || []).length;

function render(matcher: Matcher): string {
    if (matcher.kind === "wildcard") return "|" + matcher.text + "|";

    return matcher.kind === "literal" ? '"' + matcher.text + '"' : "/" + matcher.text + "/";
}

function test(matcher: Matcher, text: string): boolean {
    if (matcher.kind === "literal") return loosely(matcher.text, text, FUZZ);

    if (matcher.kind === "wildcard") {
        const chars = [...matcher.text];
        const word = /^[\p{L}\p{N}_]$/u;
        const body = chars
            .map((ch) => (ch === "*" ? ".*" : ch === "?" ? "." : ch.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&")))
            .join("");
        const start = word.test(chars[0]) ? "(?<![\\p{L}\\p{N}_])" : "";
        const end = word.test(chars[chars.length - 1]) ? "(?![\\p{L}\\p{N}_])" : "";

        return new RegExp(start + body + end, "iu").test(text);
    }

    try {
        return new RegExp(matcher.text).test(text);
    } catch {
        return false;
    }
}

function compare(value: number, cmp: string, against: number): boolean {
    return cmp === ">"
        ? value > against
        : cmp === "<"
            ? value < against
            : cmp === ">="
                ? value >= against
                : value <= against;
}

const measured = (seen: Seen, when: When) =>
    ({
        mentions: seen.mentions,
        links: seen.links,
        invites: seen.invites,
        attachments: seen.attachments,
        animated: seen.animated,
    } as Record<string, number>)[when.measure] ?? 0;

export function evaluate(rule: Body, seen: Seen): Count {
    const trace: Step[] = [];

    let stopped: string | null = null;

    if (rule.sources.includes(seen.source)) {
        trace.push({ state: "hit", what: "on " + rule.sources.join(" "), val: seen.source });
    } else {
        trace.push({ state: "stop", what: "on " + rule.sources.join(" "), val: seen.source });
        stopped = "source";
    }

    if (!stopped && rule.only.length) {
        const inside = rule.only.includes(seen.channel);

        trace.push({
            state: inside ? "hit" : "stop",
            what: "only channel:" + rule.only.join(" channel:"),
            val: inside ? "in scope" : "elsewhere",
        });

        if (!inside) stopped = "channel";
    }

    if (!stopped && rule.ignoreChannels.length) {
        const here = rule.ignoreChannels.includes(seen.channel);

        trace.push({
            state: here ? "stop" : "hit",
            what: "ignore channel:" + rule.ignoreChannels.join(" channel:"),
            val: here ? "ignored" : "not ignored",
        });

        if (here) stopped = "channel";
    }

    if (!stopped && rule.ignoreRoles.length) {
        const ignored = rule.ignoreRoles.some((role) => seen.roles.includes(role));

        trace.push({
            state: ignored ? "stop" : "hit",
            what: "ignore role:" + rule.ignoreRoles.join(" role:"),
            val: ignored ? "has role" : "no role",
        });

        if (ignored) stopped = "role";
    }

    if (!stopped && rule.ignorePermissions.length) {
        const wields = (permission: string) =>
            seen.permissions.includes("administrator") || seen.permissions.includes(permission);

        const ignored = rule.ignorePermissions.some(wields);

        trace.push({
            state: ignored ? "stop" : "hit",
            what: "ignore permission:" + rule.ignorePermissions.join(" permission:"),
            val: ignored ? "has permission" : "no permission",
        });

        if (ignored) stopped = "permission";
    }

    let caught = rule.match.length === 0;
    let cleared = false;

    if (!stopped) {
        for (const matcher of rule.match) {
            const hit = test(matcher, seen.text);

            if (hit) caught = true;

            trace.push({
                state: hit ? "hit" : "miss",
                what: "match " + render(matcher),
                val: hit ? (matcher.kind === "literal" ? "loose match" : "match") : "no match",
            });
        }

        for (const matcher of rule.never) {
            const hit = test(matcher, seen.text);

            if (hit) cleared = true;

            trace.push({
                state: hit ? "stop" : "hit",
                what: "never " + render(matcher),
                val: hit ? "excluded" : "no match",
            });
        }
    }

    let satisfied = true;

    if (!stopped)
        for (const when of rule.when) {
            let ok: boolean;
            let val: string;

            if (when.measure === "account") {
                ok = when.dir === "younger" ? seen.age < (when.secs ?? 0) : seen.age > (when.secs ?? 0);
                val = showDuration(seen.age);
            } else if (RECORD.includes(when.measure)) {
                const on = punishmentCount(seen.record, when.measure);

                ok = compare(on, when.cmp ?? ">", when.val ?? 0);
                val = String(on);
            } else {
                const on = measured(seen, when);

                ok = compare(on, when.cmp ?? ">", when.val ?? 0);
                val = String(on);
            }

            if (!ok) satisfied = false;

            trace.push({ state: ok ? "hit" : "stop", what: when.text, val });
        }

    return { trace, matched: !stopped && caught && !cleared && satisfied, stopped, cleared, caught, satisfied };
}
