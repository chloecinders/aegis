export const API = {
    identity: "/api/dash/identity",
    guild: (id: string) => "/api/dash/guilds/" + encodeURIComponent(id),
    rules: (id: string) => "/api/dash/guilds/" + encodeURIComponent(id) + "/rules",
    rule: (id: string, rule: string) => API.rules(id) + "/" + encodeURIComponent(rule),
    managed_rules: (id: string) => "/api/dash/guilds/" + encodeURIComponent(id) + "/managed_rules",
    managed: (id: string, rule: string) => API.managed_rules(id) + "/" + encodeURIComponent(rule),
    authoring: "/api/dash/managed_rules",
    authored: (rule: string) => "/api/dash/managed_rules/" + encodeURIComponent(rule),
    logs: (id: string) => "/api/dash/guilds/" + encodeURIComponent(id) + "/logs",
    errors: (id: string) => "/api/dash/guilds/" + encodeURIComponent(id) + "/errors",
    message_log: (id: string) => "/api/dash/guilds/" + encodeURIComponent(id) + "/message_log",
    permissions: (id: string) => "/api/dash/guilds/" + encodeURIComponent(id) + "/permissions",
    permission: (id: string, rule: number) => API.permissions(id) + "/" + encodeURIComponent(rule),
    commands: "/api/dash/commands",
    check: "/api/dash/check",
    activity: "/api/dash/activity",
};

export interface Entry {
    id: string;
    name: string;
}

export interface View {
    id: string;
    name: string;
    icon: string | null;
    roles: Entry[];
    channels: Entry[];
}

export interface Membership {
    id: string;
    name: string;
    icon: string | null;
}

export interface User {
    user: string;
    name: string;
    display: string | null;
    avatar: string | null;
    manages: Membership[];
    developer: boolean;
}

export interface Saved {
    id: string;
    name: string;
    mode: string;
    source: string;
    description?: string;
}

export interface Offered {
    id: string;
    name: string;
    description: string;
    sources: string[];
    offered: string;
    mode: string | null;
    effective: string;
    response: string;
    action: string | null;
}

export interface Definition {
    kind: string;
    title: string;
    about: string;
    channel: string | null;
}

export interface Trouble {
    id: number;
    headline: string;
    detail?: string;
    delivered: boolean;
    at: string;
}

export interface Built {
    id: string;
}

export interface Permission {
    id: number;
    scope: string;
    subject: string;
    target: string;
    effect: string;
    priority: number;
}

export interface Listed {
    name: string;
    category: string;
    about: string;
}

export interface Vocabulary {
    categories: string[];
    commands: Listed[];
}

export interface Error {
    problem: string;
    start?: number | null;
    len?: number | null;
}

export interface Reading {
    ok: boolean;
    error?: Error;
    rendered?: string;
}

export interface Opened {
    token: string;
    expires: string;
}

export type Refusal = "unreachable" | "anonymous" | "forbidden" | "absent" | "refused" | "broken";

export interface Answer<T> {
    value: T;
    error?: Refusal;
    detail?: Error;
}

interface Stamp {
    client: string;
}

declare global {
    interface Window {
        __AEGIS__?: Stamp;
    }
}

export const STAMP: Stamp = window.__AEGIS__ || { client: "" };
export const ACTIVITY = new URLSearchParams(location.search).has("frame_id");

let bearer: string | null = null;

export const hold = (token: string) => {
    bearer = token;
};

const carrying = (): Record<string, string> => (bearer ? { authorization: "Bearer " + bearer } : {});
const refused = <T,>(error: Refusal, detail?: Error) => ({ error, detail }) as Answer<T>;

export async function call<T>(url: string, method?: string, body?: unknown): Promise<Answer<T>> {
    let answer;

    try {
        answer = await fetch(url, {
            method: method || "GET",
            credentials: "same-origin",
            headers: body ? { "content-type": "application/json", ...carrying() } : carrying(),
            body: body ? JSON.stringify(body) : undefined,
        });
    } catch {
        return refused<T>("unreachable");
    }

    if (answer.status === 204) return { value: null as T };

    let read = null;

    try {
        read = await answer.json();
    } catch {
        read = null;
    }

    if (answer.ok) return { value: read as T };

    if (answer.status === 401) return refused<T>("anonymous");
    if (answer.status === 403) return refused<T>("forbidden");
    if (answer.status === 404) return refused<T>("absent");

    if (read && read.problem) return refused<T>("refused", read as Error);

    return refused<T>("broken");
}

export const ask = <T,>(url: string) => call<T>(url);
export const put = <T,>(url: string, body: unknown) => call<T>(url, "PUT", body);
export const post = <T,>(url: string, body: unknown) => call<T>(url, "POST", body);
export const remove = <T,>(url: string) => call<T>(url, "DELETE");

export function wording(error: Refusal | undefined): string {
    if (error === "anonymous") return "signed out";
    if (error === "forbidden") return "missing permissions";
    if (error === "absent") return "no longer exists";
    if (error === "unreachable") return "bot unreachable";

    return "save failed";
}

export const signIn = () => "/login?next=" + encodeURIComponent(location.pathname + location.search);
