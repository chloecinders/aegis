import fs from "node:fs/promises";
import path from "node:path";

import { ROOT, SRC } from "./paths.ts";
import { block, decomment, inner, item, literal, pieces, rust } from "./rust.ts";

function enabled(predicate: string, features: string[], at: string): boolean {
    const trimmed = predicate.trim();
    const call = /^(not|all|any)\s*\(/.exec(trimmed);

    if (call) {
        const parts = pieces(trimmed.slice(call[0].length - 1 + 1, -1), ",");

        if (call[1] === "not") return !enabled(parts[0]!, features, at);
        if (call[1] === "all") return parts.every((part) => enabled(part, features, at));

        return parts.some((part) => enabled(part, features, at));
    }

    const feature = /^feature\s*=\s*"/.exec(trimmed);

    if (!feature) throw new Error(`${at} is gated on \`${trimmed}\`, which the sheet cannot evaluate`);

    return features.includes(literal(trimmed, trimmed.indexOf('"')).value);
}

export interface Registered {
    commands: string[];
    slashes: string[];
}

export async function order(features: string[]): Promise<Registered> {
    const root = decomment(await rust(path.join(SRC, "features", "mod.rs")));
    const body = item(root, /pub\s+fn\s+register\s*\(/);
    const found: Registered = { commands: [], slashes: [] };

    for (const feature of body.matchAll(/([A-Za-z0-9_]+)::register\s*\(/g)) {
        const at = path.join(SRC, "features", feature[1]!, "mod.rs");
        const src = decomment(await rust(at));
        const register = item(src, /pub\s+fn\s+register\s*\(/);

        let required: string | null = null;
        let cursor = 0;

        while (cursor < register.length) {
            const next = /#\[\s*cfg\s*\(|(?:crate::)?register(_slash)?!\s*\(/.exec(register.slice(cursor));

            if (!next) break;

            const from = cursor + next.index;

            if (next[0].startsWith("#")) {
                required = inner(register, register.indexOf("(", from));
                cursor = block(register, register.indexOf("[", from));

                continue;
            }

            const opened = register.indexOf("(", from);
            const listed = pieces(inner(register, opened), ",").slice(1);

            if (!required || enabled(required, features, `${feature[1]}::register`))
                (next[1] ? found.slashes : found.commands).push(...listed.map((one) => one.split("::").pop()!.trim()));

            required = null;
            cursor = block(register, opened);
        }
    }

    return found;
}

export async function defaults(): Promise<string[]> {
    const manifest = await fs.readFile(path.join(ROOT, "Cargo.toml"), "utf8");
    const match = /^\s*default\s*=\s*\[([^\]]*)\]/m.exec(manifest);

    if (!match) throw new Error("Cargo.toml declares no default features");

    return pieces(match[1]!, ",").map((one) => one.replaceAll('"', ""));
}
