import { MemoryRouter, Route, Router, createMemoryHistory } from "@solidjs/router";
import { lazy } from "solid-js";

import { ACTIVITY } from "./api.ts";
import { Errors } from "./components/Errors.tsx";
import { Guild } from "./components/Guild.tsx";
import { Logs } from "./components/Logs.tsx";
import { MessageLog } from "./components/MessageLog.tsx";
import { Permits } from "./components/Permits.tsx";
import { Rules } from "./components/Rules.tsx";
import { Servers } from "./components/Servers.tsx";
import { Shell } from "./components/Shell.tsx";

const Editor = lazy(() => import("./editor/Editor.tsx"));
const ManagedRules = lazy(() => import("./editor/managed_rules.tsx"));

const routes = () => (
    <>
        <Route path="/dashboard" component={Shell}>
            <Route path="/" component={Servers} />

            <Route path="/:guild" component={Guild}>
                <Route path="/" component={Rules} />
                <Route path="/logs" component={Logs} />
                <Route path="/permissions" component={Permits} />
                <Route path="/errors" component={Errors} />
                <Route path="/message_log" component={MessageLog} />
            </Route>
        </Route>

        <Route path="/dashboard/managed_rules" component={ManagedRules} />
        <Route path="/dashboard/:guild/automod" component={Editor} />
    </>
);

function activityHistory() {
    const history = createMemoryHistory();

    history.set({ value: "/dashboard", replace: true });

    return history;
}

export function App() {
    return ACTIVITY ? (
        <MemoryRouter explicitLinks history={activityHistory()}>
            {routes()}
        </MemoryRouter>
    ) : (
        <Router explicitLinks>{routes()}</Router>
    );
}
