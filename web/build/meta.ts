import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import type { Documented, Flag, Sheet, SlashOption } from "../site/src/wiki/commands.ts";

import { type Modeled, commands, kinds, optionKinds, slashes } from "./meta/commands.ts";
import { defaults, order } from "./meta/features.ts";
import { SRC } from "./meta/paths.ts";
import { permissionNames, permissions } from "./meta/permissions.ts";
import { arms, decomment, item, pieces, rust, sources } from "./meta/rust.ts";

function syntax(field: Modeled, label: Map<string, string>, kind: string): string {
    const labelled = `${field.name}: ${label.get(kind)}`;

    if (field.shape === "Positional") return `<${labelled}>`;
    if (field.shape === "Optional") return `[${labelled}]`;
    if (field.shape === "Rest") return `...[${field.name}]`;
    if (field.shape === "Reply") return `(<${labelled}> || reply)`;

    return field.short ? `[+${field.short}/+${field.name}]` : `[+${field.name}]`;
}

export async function sheet(): Promise<Sheet> {
    const files = await sources(SRC);
    const args = decomment(await rust(path.join(SRC, "command", "args.rs")));
    const root = decomment(await rust(path.join(SRC, "command", "mod.rs")));

    const kind = item(args, /impl\s+ArgKind\s*\{/);
    const label = arms(item(kind, /pub\s+fn\s+label\s*\(/), "ArgKind");
    const shown = arms(item(kind, /pub\s+fn\s+example\s*\(/), "ArgKind");
    const display = arms(item(root, /impl\s+Display\s+for\s+Category\s*\{/), "Category");

    const listed = pieces(item(root, /pub\s+const\s+CATEGORIES\s*:\s*\[Category;\s*\d+\]\s*=/, "["), ",");
    const [table, structs, slashed, registered, kinded, optioned] = await Promise.all([
        permissions(),
        commands(files),
        slashes(files),
        defaults().then(order),
        kinds(files),
        optionKinds(files),
    ]);

    const written: Documented[] = registered.commands.map((struct) => {
        const parsed = structs.get(struct);

        if (!parsed) throw new Error(`${struct} is registered but is not a #[command] struct`);
        if (!display.has(parsed.category)) throw new Error(`${struct} sits in an unknown category ${parsed.category}`);

        const resolved = parsed.fields.map((field) => {
            const found = kinded.get(field.inner);

            if (!found) throw new Error(`${struct}.${field.name} is typed ${field.inner}, which has no FromArgs impl`);
            if (!label.has(found)) throw new Error(`ArgKind::${found} has no label`);

            return { field, kind: found };
        });

        const flags: Flag[] = resolved
            .filter(({ field }) => field.shape === "Flag")
            .map(({ field }) => ({
                name: field.name,
                switch: field.short ? `+${field.short}/+${field.name}` : `+${field.name}`,
                desc: field.desc,
            }));

        return {
            name: parsed.name,
            aliases: parsed.aliases,
            short: parsed.short,
            full: parsed.full,
            category: display.get(parsed.category)!,
            developer: parsed.developer,
            hidden: parsed.hidden,
            syntax: resolved
                .filter(({ field }) => field.shape !== "Flag")
                .map(({ field, kind }) => syntax(field, label, kind))
                .join(" "),
            example: resolved
                .filter(({ field }) => field.shape !== "Flag")
                .map(({ kind }) => shown.get(kind)!)
                .join(" "),
            user: permissionNames(table, parsed.user, parsed.name),
            one_of: permissionNames(table, parsed.one_of, parsed.name),
            flags,
            slash: null,
        };
    });

    for (const struct of registered.slashes) {
        const parsed = slashed.get(struct);

        if (!parsed) throw new Error(`${struct} is registered but is not a #[slash] struct`);
        if (!display.has(parsed.category)) throw new Error(`${struct} sits in an unknown category ${parsed.category}`);

        const options: SlashOption[] = parsed.fields.map((field) => {
            const kind = optioned.get(field.inner);

            if (!kind) throw new Error(`${struct}.${field.name} is typed ${field.inner}, which has no FromOption impl`);

            return { name: field.name, kind, desc: field.desc, required: field.shape === "Positional" };
        });

        const shared = written.find((one) => one.name === parsed.name);

        if (shared) {
            shared.slash = options;

            continue;
        }

        written.push({
            name: parsed.name,
            aliases: parsed.aliases,
            short: parsed.short,
            full: parsed.full,
            category: display.get(parsed.category)!,
            developer: parsed.developer,
            hidden: parsed.hidden,
            syntax: null,
            example: null,
            user: permissionNames(table, parsed.user, parsed.name),
            one_of: permissionNames(table, parsed.one_of, parsed.name),
            flags: [],
            slash: options,
        });
    }

    return {
        categories: listed.map((one) => {
            const variant = one.replace("Category::", "").trim();

            if (!display.has(variant)) throw new Error(`CATEGORIES lists an unknown ${variant}`);

            return display.get(variant)!;
        }),
        commands: written,
    };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    const at = process.argv[2];

    if (!at) throw new Error("meta.ts needs a path to write to");

    const dumped = await sheet();

    await fs.writeFile(at, JSON.stringify(dumped, null, 2) + "\n");

    console.log(`dumped ${dumped.commands.length} commands`);
}
