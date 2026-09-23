import { For, Show } from "solid-js";

import type { Clause } from "../grammar.ts";
import { useEditor } from "../state/editor.tsx";

export function ClauseDetail(props: { clause: Clause; back: string }) {
    const { editing } = useEditor();

    return (
        <>
            <div class="colhead colhead--teal">
                <span>{props.clause.keyword}</span>
            </div>

            <div>
                <button class="back" onClick={editing.unpick}>
                    {props.back}
                </button>

                <div class="hint">{props.clause.about}</div>

                <Show
                    when={props.clause.values.length}
                    fallback={
                        <div class="hint" style="border-top:1px solid var(--rule)">
                            nothing
                        </div>
                    }
                >
                    <div class="vocab vocab--values">
                        <For each={props.clause.values}>
                            {([value, about]) => (
                                <button class="vocab__item">
                                    <span class="vocab__key">{value}</span>
                                    <span class="vocab__takes">{about}</span>
                                </button>
                            )}
                        </For>
                    </div>
                </Show>
            </div>
        </>
    );
}
