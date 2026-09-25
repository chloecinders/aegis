import { A, useParams } from "@solidjs/router";
import type { Accessor, JSX } from "solid-js";
import { createContext, createResource, Show, useContext } from "solid-js";

import type { View } from "../api.ts";
import { API, ask } from "../api.ts";
import { Crest } from "./Chrome.tsx";
import { Explain } from "./Note.tsx";

const guildContext = createContext<Accessor<View>>();

export function useGuild(): Accessor<View> {
    const ctx = useContext(guildContext);

    if (!ctx) throw new Error("a guild view is only available inside the guild route");

    return ctx;
}

export const editor = (guild: string) => "/dashboard/" + encodeURIComponent(guild) + "/automod";

const tab = (guild: string, name: string) => "/dashboard/" + encodeURIComponent(guild) + name;

function Tabs(props: { guild: string }) {
    return (
        <nav class="tabs">
            <A class="tabs__tab" href={tab(props.guild, "")} end activeClass="tabs__tab--on">
                rules
            </A>
            <A class="tabs__tab" href={tab(props.guild, "/logs")} activeClass="tabs__tab--on">
                logs
            </A>
            <A class="tabs__tab" href={tab(props.guild, "/permissions")} activeClass="tabs__tab--on">
                permissions
            </A>
            <A class="tabs__tab" href={tab(props.guild, "/errors")} activeClass="tabs__tab--on">
                errors
            </A>
            <A class="tabs__tab" href={tab(props.guild, "/message_log")} activeClass="tabs__tab--on">
                message log
            </A>
        </nav>
    );
}

export function Guild(props: { children?: JSX.Element }) {
    const params = useParams();
    const [view] = createResource(
        () => params.guild,
        (guild) => ask<View>(API.guild(guild)),
    );

    return (
        <Show when={view()}>
            {(loaded) => (
                <Show when={!loaded().error} fallback={<Explain error={loaded().error} />}>
                    <div class="band">
                        <Crest name={loaded().value.name} icon={loaded().value.icon} />
                        <div>
                            <h1 class="band__title">{loaded().value.name}</h1>
                        </div>
                        <div class="band__acts">
                            <A class="dashboard__button dashboard__button--line" href="/dashboard">
                                all servers
                            </A>
                        </div>
                    </div>

                    <Tabs guild={loaded().value.id} />

                    <guildContext.Provider value={() => loaded().value}>{props.children}</guildContext.Provider>
                </Show>
            )}
        </Show>
    );
}
