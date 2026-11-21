import type { Clause } from "../rule.ts";
import { bareChannel, idOf, permissionOf } from "../tokenise.ts";

export function readScope(clause: Clause): void {
    if (clause.rest.length === 0) {
        if (clause.word === "only") clause.bad("missing channel", "provide a channel", "only channel:<id>");
        else clause.bad("missing role, user, channel or permission", "provide one", "ignore role:<id>");

        return;
    }

    for (const token of clause.rest) {
        const channel = idOf(token, "channel");
        const role = idOf(token, "role");

        if (clause.word === "ignore" && token.text.toLowerCase().startsWith("permission:")) {
            const permission = permissionOf(token);

            if (permission) {
                if (!clause.body.ignorePermissions.includes(permission)) clause.body.ignorePermissions.push(permission);
            } else {
                clause.bad("no permission found", "provide a valid permission", "permission:manage_messages");
            }

            continue;
        }

        if (clause.word === "only") {
            if (role) {
                clause.bad("found role, expected channel", "provide a channel", "channel:<id>");
            } else {
                const only = channel || bareChannel(token);

                if (only) {
                    if (!clause.body.only.includes(only)) clause.body.only.push(only);
                } else {
                    clause.bad("expected channel:<id>", "provide a channel", "channel:<id>");
                }
            }

            continue;
        }

        const user = idOf(token, "user");

        if (user) {
            if (!clause.body.ignoreUsers.includes(user)) clause.body.ignoreUsers.push(user);
        } else if (channel) {
            if (!clause.body.ignoreChannels.includes(channel)) clause.body.ignoreChannels.push(channel);
        } else if (role) {
            if (!clause.body.ignoreRoles.includes(role)) clause.body.ignoreRoles.push(role);
        } else {
            clause.bad("expected role:<id>, user:<id>, channel:<id> or permission:<name>", "provide one", "role:<id>");
        }
    }
}
