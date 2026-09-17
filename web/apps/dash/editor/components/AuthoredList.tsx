import { For, Show } from "solid-js";

import { parse } from "../parse.ts";
import { useEditor } from "../state/editor.tsx";

export function AuthoredList() {
    const { session, focus, editing, rules } = useEditor();

    return (
        <div class="editor__col">
            <div class="colhead">
                <span>managed rules</span>
                <span>{rules.list.length}</span>
            </div>

            <div class="rulelist">
                <For each={rules.list}>
                    {(rule) => {
                        const open = () => rule.id === focus.rule();
                        const parsed = () => parse(open() ? editing.draft() : rule.source, "detection");

                        return (
                            <a
                                class={open() ? "rulelist__item rulelist__item--on" : "rulelist__item"}
                                onClick={() => rules.switchTo(rule.id)}
                            >
                                <span class="rulelist__name">
                                    {rule.name}
                                    <Show when={open() && rules.dirty()}> <em class="rulelist__dirty">*</em></Show>
                                </span>
                                <span class="rulelist__meta">
                                    <span class={"rulelist__state rulelist__state--" + rule.mode}>{rule.mode}</span>
                                    <span class="rulelist__bad">
                                        {parsed().errors ? parsed().errors + " err" : ""}
                                    </span>
                                </span>
                            </a>
                        );
                    }}
                </For>
            </div>

            <button class="new-rule" disabled={session.busy()} onClick={rules.create}>
                + new managed rule
            </button>
        </div>
    );
}
