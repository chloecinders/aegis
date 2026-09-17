const PARTS = location.pathname.split("/").filter(Boolean).slice(-2);
const BASE = "/api/transcript/" + PARTS.map(encodeURIComponent).join("/");
export const GUILD = PARTS[0] ?? "";

export interface Entry {
    id: string;
    name: string;
}

export interface Viewer {
    name: string;
    display: string | null;
    avatar: string | null;
}

export interface Revision {
    content: string;
    at: string;
}

export interface Rendered {
    id: string;
    channel: string;
    author: string;
    name: string;
    display: string | null;
    avatar: string | null;
    reply_to?: string | null;
    reply_collected?: boolean;
    content: string;
    files?: string[];
    at: string;
    removed?: boolean;
    system?: boolean;
    edits?: Revision[];
}

export type Scope = "channel" | "user" | "cleared" | "selection";

export interface Meta {
    id: string;
    guild: number;
    scope: Scope;
    channel: number | null;
    channel_name: string | null;
    subject: number | null;
    subject_name: string | null;
    window_start: string | null;
    window_end: string | null;
    moderator_name: string;
    created_at: string;
    total: number;
}

export interface Header extends Meta {
    title: string;
    spans_channels: boolean;
    jumpable: boolean;
    channels: Entry[];
}

export interface Answer {
    next: string | null;
    messages: Rendered[];
}

export class Refused extends Error {
    status: number;

    constructor(status: number) {
        super(`the server answered ${status}`);

        this.status = status;
    }
}

async function read<T>(url: string): Promise<T> {
    const answer = await fetch(url);

    if (!answer.ok) throw new Refused(answer.status);

    return answer.json() as Promise<T>;
}

export const meta = () => read<Header>(BASE);
export const viewer = () => read<Viewer>("/api/dash/identity");

export const messages = (before?: string | null) =>
    read<Answer>(BASE + "/messages" + (before ? `?before=${encodeURIComponent(before)}` : ""));
