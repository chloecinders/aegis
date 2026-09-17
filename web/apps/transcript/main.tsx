import { For, Show, createEffect, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { render } from "solid-js/web";

import { Account, Footer, Top } from "../../shared/chrome.tsx";
import type { Answer, Header, Rendered, Viewer } from "./api.ts";
import { Refused, messages, meta, viewer } from "./api.ts";
import { Message } from "./components/Message.tsx";

interface Note {
    title: string;
    body?: string;
    back?: string;
}

interface Line {
    grouped: boolean;
    opens: boolean;
}

function refusal(status: number): Note {
    if (status === 401)
        return {
            title: "Sign in required",
            body: "You do not have the required permissions to read this transcript.",
            back: "/login?next=" + encodeURIComponent(location.pathname),
        };

    if (status === 403)
        return {
            title: "Not permitted",
            body: "You do not have the required permissions to read this transcript.",
        };

    return { title: "Transcript not found" };
}

function Head(props: { meta: Header | null; viewer: Viewer | null; fetched: boolean }) {
    const meta = () => props.meta;

    return (
        <header class="head">
            <div class="page">
                <Top crumb={<span class="head__kind">Transcript</span>}>
                    <Show when={props.viewer}>{(account) => <Account viewer={account()} />}</Show>
                </Top>

                <div class="band">
                    <h1 class="band__title">{meta()?.title || "Transcript"}</h1>

                    <div class="static">
                        <Show
                            when={meta()}
                            fallback={
                                <Show when={!props.fetched}>
                                    <span class="static__item">Loading</span>
                                </Show>
                            }
                        >
                            {(head) => (
                                <span class="static__item">
                                    Saved <b class="static__value">{new Date(head().created_at).toLocaleString()}</b>
                                </span>
                            )}
                        </Show>
                    </div>
                </div>
            </div>
        </header>
    );
}

function Said(props: { note: Note }) {
    return (
        <div class="note">
            <h2 class="note__title">{props.note.title}</h2>
            <Show when={props.note.body}>
                <p class="note__body">{props.note.body}</p>
            </Show>

            <Show when={props.note.back}>
                {(back) => (
                    <a class="note__link" href={back()}>
                        Sign in with Discord
                    </a>
                )}
            </Show>
        </div>
    );
}

function Split(props: { name: string }) {
    return (
        <div class="split">
            <span>#{props.name}</span>
        </div>
    );
}

function bottom() {
    scrollTo(0, document.documentElement.scrollHeight);
}

function Transcript() {
    const [head, setHead] = createSignal<Header | null>(null);
    const [account, setAccount] = createSignal<Viewer | null>(null);
    const [note, setNote] = createSignal<Note | null>(null);
    const [fetched, setFetched] = createSignal(false);
    const [feed, setFeed] = createSignal<Rendered[]>([]);
    const [ended, setEnded] = createSignal(false);
    const [loading, setLoading] = createSignal(false);

    let before: string | null = null;
    let fetching: Promise<void> | null = null;

    const names = createMemo(() => new Map((head()?.channels || []).map((channel) => [channel.id, channel.name])));

    const lines = createMemo(() => {
        const placed = new Map<string, Line>();

        let anchor: Rendered | null = null;
        let channel: string | null = null;

        for (const message of feed()) {
            const grouped =
                anchor !== null &&
                !message.system &&
                anchor.author === message.author &&
                anchor.channel === message.channel &&
                new Date(message.at).getTime() - new Date(anchor.at).getTime() < 300000;

            if (!grouped) anchor = message;

            placed.set(message.id, { grouped, opens: message.channel !== channel });

            channel = message.channel;
        }

        return placed;
    });

    async function load() {
        setLoading(true);

        while (!ended()) {
            let page: Answer;

            try {
                page = await messages(before);
            } catch {
                setNote({ title: "Load failed" });
                setEnded(true);

                break;
            }

            const height = document.documentElement.scrollHeight;

            setFeed((current) => [...page.messages, ...current]);

            before = page.next;

            setEnded(page.next === null || page.next === undefined);

            scrollTo(0, scrollY + document.documentElement.scrollHeight - height);

            if (document.body.scrollHeight > innerHeight) break;
        }

        setLoading(false);
    }

    function more(): Promise<void> {
        if (ended()) return Promise.resolve();

        if (!fetching) fetching = load().finally(() => (fetching = null));

        return fetching;
    }

    async function reveal(id: string) {
        while (!document.getElementById(`m${id}`) && !ended()) await more();

        location.hash = `m${id}`;
    }

    onMount(async () => {
        viewer()
            .then(setAccount)
            .catch(() => setAccount(null));

        let found: Header;

        try {
            found = await meta();
        } catch (failure) {
            setNote(refusal(failure instanceof Refused ? failure.status : 0));
            setFetched(true);

            return;
        }

        setHead(found);
        setFetched(true);

        await more();

        bottom();
        requestAnimationFrame(bottom);
    });

    const scrolled = () => {
        if (scrollY <= 800) more();
    };

    addEventListener("scroll", scrolled, { passive: true });
    onCleanup(() => removeEventListener("scroll", scrolled));

    createEffect(() => {
        const title = head()?.title;

        if (title) document.title = "Transcript - " + title;
    });

    return (
        <div class="transcript">
            <div class="slab" />

            <Head meta={head()} viewer={account()} fetched={fetched()} />

            <main class="page">
                <Show when={loading()}>
                    <div class="status">Loading</div>
                </Show>

                <Show when={note()}>{(said) => <Said note={said()} />}</Show>

                <div class="transcript__feed">
                    <For each={feed()}>
                        {(message) => [
                            <Show when={head()?.spans_channels && lines().get(message.id)?.opens}>
                                <Split name={names().get(message.channel) || message.channel} />
                            </Show>,
                            <Message
                                message={message}
                                grouped={lines().get(message.id)?.grouped}
                                jumpable={head()?.jumpable}
                                reveal={reveal}
                            />,
                        ]}
                    </For>
                </div>
            </main>

            <Footer class="page transcript__footer" />
        </div>
    );
}

const root = document.getElementById("app");

if (!root) throw new Error("no #app found");

render(Transcript, root);
