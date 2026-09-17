import { Show } from "solid-js";

import { AuthoredList } from "./components/AuthoredList.tsx";
import { Frame } from "./components/Chrome.tsx";
import { Clauses } from "./components/Clauses.tsx";
import { Description } from "./components/Description.tsx";
import { NoRules } from "./components/NoRules.tsx";
import { Preview } from "./components/Preview.tsx";
import { Vocab } from "./components/Vocab.tsx";
import { EditorProvider, useEditor } from "./state/editor.tsx";

function Work() {
    const { rules } = useEditor();

    return (
        <Frame>
            <AuthoredList />

            <Show when={rules.open()} fallback={<NoRules />}>
                <div class="editor__col">
                    <Description />
                    <Clauses />
                    <Preview />
                </div>

                <Vocab />
            </Show>
        </Frame>
    );
}

export default function ManagedRules() {
    return (
        <EditorProvider mode="authored" guild="">
            <Work />
        </EditorProvider>
    );
}
