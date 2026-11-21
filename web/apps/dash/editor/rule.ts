import type { Matcher, Token } from "./tokenise.ts";

export interface Diag {
    line: number;
    level: "error" | "warn";
    msg: string;
    help?: string;
    fill?: string;
}

export interface When {
    measure: string;
    line: number;
    text: string;
    dir?: "younger" | "older";
    secs?: number | null;
    cmp?: string;
    val?: number;
    within?: number | null;
}

export interface Then {
    verb: string;
    secs: number | null;
}

export interface After {
    count: number;
    secs: number;
}

export interface Body {
    sources: string[];
    match: Matcher[];
    never: Matcher[];
    when: When[];
    only: string[];
    ignoreRoles: string[];
    ignoreUsers: string[];
    ignoreChannels: string[];
    ignorePermissions: string[];
    after: After | null;
    then: Then | null;
    delete: boolean;
    clear: number | null;
    notify: string | null;
    reason: string | null;
}

export interface Parsed {
    body: Body;
    diags: Diag[];
    errors: number;
}

export type Part = "detection" | "response" | "whole";

export interface Clause {
    body: Body;
    seen: Record<string, number>;
    line: number;
    text: string;
    head: Token;
    rest: Token[];
    word: string;
    bad(msg: string, help?: string, fill?: string): void;
    warn(msg: string): void;
    once(msg: string): boolean;
}
