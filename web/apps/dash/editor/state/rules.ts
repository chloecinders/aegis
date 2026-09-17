import type { Accessor } from "solid-js";
import { createStore, produce } from "solid-js/store";

import type { Saved } from "../../api.ts";
import { call } from "../../api.ts";
import { ask, discard } from "../ask.ts";
import type { Editing } from "./editing.ts";
import type { Focus } from "./focus.ts";
import type { Session } from "./session.ts";
import { trouble } from "./session.ts";

interface Stored {
    id: string;
    name: string;
    mode: string;
    source: string;
    description: string;
}

export interface Rule extends Stored {
    saved: Stored;
}

interface Written {
    name: string;
    mode: string;
    source: string;
    description?: string;
}

export interface Rules {
    list: Rule[];
    open: Accessor<Rule | undefined>;
    dirty: Accessor<boolean>;
    rename: (name: string) => void;
    enable: (mode: string) => void;
    describe: (description: string) => void;
    take: (rules: Saved[]) => void;
    first: (asked?: string) => string | null;
    show: (id: string | null) => void;
    switchTo: (id: string) => Promise<void>;
    commit: () => Promise<void>;
    revert: () => void;
    remove: () => Promise<void>;
    create: () => Promise<void>;
}

const stored = (rule: Saved): Stored => ({
    id: rule.id,
    name: rule.name,
    mode: rule.mode,
    source: rule.source,
    description: rule.description || "",
});

export const adopt = (rule: Saved): Rule => Object.assign(stored(rule), { saved: stored(rule) });

export function createRules(session: Session, focus: Focus, editing: Editing): Rules {
    const [list, setList] = createStore<Rule[]>([]);

    const open = () => list.find((rule) => rule.id === focus.rule());
    const index = () => list.findIndex((rule) => rule.id === focus.rule());

    function dirty() {
        const rule = open();

        if (!rule) return false;

        return (
            editing.draft() !== rule.saved.source ||
            rule.name !== rule.saved.name ||
            rule.mode !== rule.saved.mode ||
            rule.description !== rule.saved.description
        );
    }

    function show(id: string | null) {
        focus.onRule(id);
        editing.load(list.find((rule) => rule.id === id)?.source ?? "", session.part);
        session.surface.name?.blur();
        session.say(null);

        if (id) editing.check();
    }

    async function leaving() {
        const rule = open();

        return !dirty() || !rule || (await discard(rule.name));
    }

    function unused(): string {
        const taken = new Set(list.map((rule) => rule.name.toLowerCase()));

        if (!taken.has("untitled")) return "untitled";

        for (let n = 2; ; n++) {
            if (!taken.has("untitled-" + n)) return "untitled-" + n;
        }
    }

    async function commit() {
        const rule = open();

        if (!rule || session.busy() || !dirty()) return;

        const written: Written = { name: rule.name, mode: rule.mode, source: editing.draft() };

        if (session.authored) written.description = rule.description;

        session.say("saving", "");

        const answer = await session.during(() => call<Saved>(session.where.one(rule.id), "PUT", written));

        if (answer.error) {
            if (answer.error === "refused") editing.blame(answer.detail);

            return session.say(trouble(answer), "bad");
        }

        setList(
            index(),
            produce((slot) => {
                slot.source = written.source;
                slot.name = answer.value.name;
                slot.mode = answer.value.mode;
                slot.description = answer.value.description || "";
                slot.saved = stored({ ...slot, source: written.source });
            }),
        );

        session.say("saved", "");
        editing.check();
    }

    function revert() {
        const rule = open();

        if (!rule) return;

        const back = rule.saved;

        setList(
            index(),
            produce((slot) => {
                slot.name = back.name;
                slot.mode = back.mode;
                slot.source = back.source;
                slot.description = back.description;
            }),
        );

        editing.load(back.source, session.part);
        session.say(null);
        editing.check();
    }

    async function remove() {
        const rule = open();

        if (!rule || session.busy()) return;

        const verb = session.authored ? "retire" : "delete";

        if (!(await ask({ headline: verb + " " + rule.name + "?", confirm: verb, danger: true }))) return;

        const answer = await session.during(() => call<null>(session.where.one(rule.id), "DELETE"));

        if (answer.error) return session.say(trouble(answer), "bad");

        setList((all) => all.filter((one) => one.id !== rule.id));
        show(list.length ? list[0].id : null);
        session.say("deleted " + rule.name, "");
    }

    async function create() {
        if (session.busy() || !(await leaving())) return;

        const written: Written = { name: unused(), mode: "disabled", source: session.seed };

        if (session.authored) written.description = "";

        session.say("creating", "");

        const answer = await session.during(() => call<Saved>(session.where.all, "POST", written));

        if (answer.error) return session.say(trouble(answer), "bad");

        setList(list.length, adopt(answer.value));
        show(answer.value.id);
        session.surface.name?.focus();
    }

    return {
        list,
        open,
        dirty,
        rename: (name) => setList(index(), "name", name),
        enable: (mode) => setList(index(), "mode", mode),
        describe: (description) => setList(index(), "description", description),
        take: (rules) => setList(rules.map(adopt)),
        first: (asked) => (asked && list.some((one) => one.id === asked) ? asked : (list[0]?.id ?? null)),
        show,
        async switchTo(id) {
            if (id === focus.rule() || !(await leaving())) return;

            show(id);
        },
        commit,
        revert,
        remove,
        create,
    };
}
