import { For, Show } from "solid-js";

import { CLAUSES } from "../grammar.ts";
import { useEditor } from "../state/editor.tsx";
import { ClauseDetail } from "./ClauseDetail.tsx";

export function Vocab() {
    const { editing } = useEditor();

    const clause = () => CLAUSES.find((c) => c.keyword === editing.vocab());

    return (
        <div class="editor__col">
            <Show
                when={clause()}
                fallback={
                    <>
                        <div class="colhead">
                            <span>clauses</span>
                        </div>

                        <div>
                            <div class="vocab">
                                <For each={CLAUSES}>
                                    {(c) => (
                                        <button class="vocab__item" onClick={() => editing.pick(c.keyword)}>
                                            <span class="vocab__key">{c.keyword}</span>
                                            <span class="vocab__takes">{c.takes}</span>
                                        </button>
                                    )}
                                </For>
                            </div>
                        </div>
                    </>
                }
            >
                {(picked) => <ClauseDetail clause={picked()} back="all clauses" />}
            </Show>
        </div>
    );
}
