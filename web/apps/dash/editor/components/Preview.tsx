import { For, Show } from "solid-js";

import { showDuration } from "../duration.ts";
import type { Step } from "../evaluate.ts";
import { punishmentCount } from "../evaluate.ts";
import { KEPT, PERMISSIONS, SOURCES } from "../grammar.ts";
import { useEditor } from "../state/editor.tsx";

function Derived() {
    const { preview } = useEditor();

    const shown = (): [string, string | number][] => {
        const observed = preview.observed();

        return [
            ["mentions", observed.mentions],
            ["links", observed.links],
            ["invites", observed.invites],
            ["attachments", observed.attachments],
            ["animated", observed.animated],
            ["age", showDuration(observed.age)],
            ["punishments", punishmentCount(observed.record, "punishments")],
        ];
    };

    return (
        <div class="derived">
            <For each={shown()}>
                {([label, value]) => (
                    <span class="derived__item">
                        {label} <b class="derived__value">{value}</b>
                    </span>
                )}
            </For>
        </div>
    );
}

function Trace() {
    const { editing, rules } = useEditor();

    const trace = () => editing.result().trace;
    const hit = () => trace().filter((t) => t.state === "hit").length;

    const mark = (state: Step["state"]) => (state === "hit" ? "✓" : state === "stop" ? "x" : "–");

    return (
        <>
            <div class="colhead colhead--grey">
                <span>trace</span>
                <span>
                    <Show when={rules.open() && trace().length}>
                        {(editing.result().matched ? "match" : "no match") + " · " + hit() + " / " + trace().length}
                    </Show>
                </span>
            </div>

            <div class="trace">
                <Show when={rules.open()} fallback={<div class="trace__none">no rule open</div>}>
                    <Show when={trace().length} fallback={<div class="trace__none">no clauses</div>}>
                        <For each={trace()}>
                            {(t) => (
                                <div class={"trace__row trace__row--" + t.state}>
                                    <span class="trace__mark">{mark(t.state)}</span>
                                    <span class="trace__what">{t.what}</span>
                                    <span class="trace__value">{t.val}</span>
                                </div>
                            )}
                        </For>
                    </Show>
                </Show>
            </div>
        </>
    );
}

export function Preview() {
    const { session, preview } = useEditor();

    const channels = () => {
        const found = session.view()?.channels ?? [];

        return found.length
            ? found.map((one) => ({ id: one.id, label: "#" + one.name }))
            : [{ id: "0", label: "no channels found" }];
    };

    return (
        <div class="preview">
            <div class="preview__side">
                <div class="colhead colhead--teal">
                    <span>sample</span>
                </div>

                <div class="pad">
                    <textarea
                        class="preview__sample"
                        spellcheck={false}
                        disabled={preview.source() === "join"}
                        value={preview.sample()}
                        onInput={(e) => preview.setSample(e.currentTarget.value)}
                    />

                    <span class="preview__label">source</span>

                    <div class="chips">
                        <For each={SOURCES}>
                            {(source) => (
                                <button
                                    class={[
                                        "chips__chip",
                                        preview.source() === source ? "chips__chip--on" : "",
                                        preview.source() === source && source === "join" ? "chips__chip--warn" : "",
                                    ]
                                        .filter(Boolean)
                                        .join(" ")}
                                    onClick={() => preview.setSource(source)}
                                >
                                    {source}
                                </button>
                            )}
                        </For>
                    </div>

                    <span class="preview__label">author and message</span>

                    <div class="fields">
                        <div class="fields__field">
                            <label class="fields__label" for="age">account age</label>
                            <select
                                class="fields__select"
                                id="age"
                                value={preview.age()}
                                onChange={(e) => preview.setAge(e.currentTarget.value)}
                            >
                                <For
                                    each={[
                                        ["3d", "3 days"],
                                        ["7d", "7 days"],
                                        ["30d", "30 days"],
                                        ["1y", "1 year"],
                                    ]}
                                >
                                    {([value, label]) => <option value={value}>{label}</option>}
                                </For>
                            </select>
                        </div>

                        <div class="fields__field">
                            <label class="fields__label" for="atts">attachments</label>
                            <input
                                class="fields__input"
                                id="atts"
                                type="number"
                                min="0"
                                max="10"
                                value={preview.atts()}
                                onInput={(e) => preview.setAtts(e.currentTarget.value)}
                            />
                        </div>

                        <div class="fields__field">
                            <label class="fields__label" for="animated">animated</label>
                            <input
                                class="fields__input"
                                id="animated"
                                type="number"
                                min="0"
                                max="10"
                                value={preview.animated()}
                                onInput={(e) => preview.setAnimated(e.currentTarget.value)}
                            />
                        </div>

                        <div class="fields__field">
                            <label class="fields__label" for="roles">role</label>
                            <select
                                class="fields__select"
                                id="roles"
                                value={preview.role()}
                                onChange={(e) => preview.setRole(e.currentTarget.value)}
                            >
                                <option value="none">none</option>
                                <For each={session.view()?.roles ?? []}>
                                    {(role) => <option value={role.id}>{role.name}</option>}
                                </For>
                            </select>
                        </div>

                        <div class="fields__field">
                            <label class="fields__label" for="perm">permission</label>
                            <select
                                class="fields__select"
                                id="perm"
                                value={preview.permission()}
                                onChange={(e) => preview.setPermission(e.currentTarget.value)}>
                                <option value="none">none</option>
                                <For each={PERMISSIONS}>{(permission) => <option value={permission}>{permission}</option>}</For>
                            </select>
                        </div>

                        <div class="fields__field">
                            <label class="fields__label" for="chan">channel</label>
                            <select
                                class="fields__select"
                                id="chan"
                                value={preview.channel()}
                                onChange={(e) => preview.setChannel(e.currentTarget.value)}
                            >
                                <For each={channels()}>{(channel) => <option value={channel.id}>{channel.label}</option>}</For>
                            </select>
                        </div>
                    </div>

                    <span class="preview__label">log counts within the rule window</span>

                    <div class="fields">
                        <For each={KEPT}>
                            {(measure) => (
                                <div class="fields__field">
                                    <label class="fields__label" for={measure}>{measure}</label>
                                    <input
                                        class="fields__input"
                                        id={measure}
                                        type="number"
                                        min="0"
                                        max="99"
                                        value={preview.filed()[measure]}
                                        onInput={(e) => preview.file(measure, e.currentTarget.value)}
                                    />
                                </div>
                            )}
                        </For>
                    </div>

                    <Derived />
                </div>

                <Trace />
            </div>
        </div>
    );
}
