import { createSignal } from "solid-js";

import type { Built } from "../api.ts";
import { API, post, wording } from "../api.ts";
import { useGuild } from "./Guild.tsx";

interface Said {
    text: string;
    kind: string;
}

export function MessageLog() {
    const guild = useGuild();
    const [user, setUser] = createSignal("");
    const [busy, setBusy] = createSignal(false);
    const [said, setSaid] = createSignal<Said | null>(null);

    async function open() {
        if (!user()) {
            setSaid({ text: "provide a user id", kind: "bad" });
            return;
        }

        setBusy(true);
        setSaid({ text: "building", kind: "" });

        const answer = await post<Built>(API.message_log(guild().id), { user: user() });

        setBusy(false);

        if (answer.error) {
            setSaid({ text: answer.detail ? answer.detail.problem : wording(answer.error), kind: "bad" });
            return;
        }

        location.assign("/transcript/" + encodeURIComponent(guild().id) + "/" + encodeURIComponent(answer.value.id));
    }

    return (
        <>
            <div class="section">
                <span>message log</span>
            </div>

            <div class="grant">
                <div class="grant__line">
                    <label class="grant__label grant__label--wide">
                        <span class="grant__title">user</span>
                        <input
                            class="grant__input"
                            value={user()}
                            placeholder="user id"
                            inputmode="numeric"
                            onInput={(e) => setUser(e.currentTarget.value.trim())}
                            onKeyDown={(e) => e.key === "Enter" && open()}
                        />
                    </label>
                </div>

                <div class="grant__line grant__line--end">
                    <span class={said() ? "grant__said grant__said--" + said()?.kind : "grant__said"}>{said() ? said()?.text : ""}</span>

                    <button class="dashboard__button" disabled={busy()} onClick={open}>
                        open
                    </button>
                </div>
            </div>
        </>
    );
}
