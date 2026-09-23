import type { Accessor, Setter } from "solid-js";
import { createMemo, createSignal } from "solid-js";

import { parseDuration } from "../duration.ts";
import type { Seen } from "../evaluate.ts";
import { INVITES, LINKS, MENTIONS, count } from "../evaluate.ts";
import type { Session } from "./session.ts";

export interface Preview {
    sample: Accessor<string>;
    setSample: Setter<string>;
    source: Accessor<string>;
    setSource: Setter<string>;
    age: Accessor<string>;
    setAge: Setter<string>;
    atts: Accessor<string>;
    setAtts: Setter<string>;
    animated: Accessor<string>;
    setAnimated: Setter<string>;
    role: Accessor<string>;
    setRole: Setter<string>;
    permission: Accessor<string>;
    setPermission: Setter<string>;
    channel: Accessor<string>;
    setChannel: Setter<string | null>;
    filed: Accessor<Record<string, string>>;
    file: (kind: string, count: string) => void;
    observed: Accessor<Seen>;
}

export function createPreview(session: Session): Preview {
    const [sample, setSample] = createSignal("free nitro, claim it here");
    const [source, setSource] = createSignal("content");
    const [age, setAge] = createSignal("30d");
    const [atts, setAtts] = createSignal("0");
    const [animated, setAnimated] = createSignal("0");
    const [role, setRole] = createSignal("none");
    const [permission, setPermission] = createSignal("none");
    const [picked, setChannel] = createSignal<string | null>(null);
    const [filed, setFiled] = createSignal<Record<string, string>>({
        warns: "0",
        mutes: "0",
        kicks: "0",
        bans: "0",
    });

    const channel = () => picked() ?? session.view()?.channels[0]?.id ?? "0";

    const observed = createMemo<Seen>(() => {
        const from = source();
        const joining = from === "join";
        const written = joining ? "" : sample();
        const text = joining || from === "message" ? "" : written;
        const chosen = role();
        const wields = permission();

        return {
            source: from,
            text,
            channel: channel(),
            roles: chosen && chosen !== "none" ? [chosen] : [],
            permissions: wields && wields !== "none" ? [wields] : [],
            age: parseDuration(age()) || 0,
            mentions: count(written, MENTIONS),
            links: count(written, LINKS),
            invites: count(written, INVITES),
            attachments: joining ? 0 : Number(atts() || 0),
            animated: joining ? 0 : Number(animated() || 0),
            record: filed(),
        };
    });

    return {
        sample,
        setSample,
        source,
        setSource,
        age,
        setAge,
        atts,
        setAtts,
        animated,
        setAnimated,
        role,
        setRole,
        permission,
        setPermission,
        channel,
        setChannel,
        filed,
        file: (kind, count) => setFiled({ ...filed(), [kind]: count }),
        observed,
    };
}
