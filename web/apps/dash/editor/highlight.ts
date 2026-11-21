import { DUR } from "./duration.ts";
import { CMPS, KEYWORDS } from "./grammar.ts";
import type { Diag } from "./rule.ts";
import { tokenise } from "./tokenise.ts";

const esc = (s: string) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

export function highlight(source: string, diags: Diag[]): string {
    const byLine: Record<number, Diag> = {};

    for (const d of diags) if (d.line && !byLine[d.line]) byLine[d.line] = d;

    return source
        .split("\n")
        .map((line, index) => {
            const d = byLine[index + 1];
            let painted: string;

            if (line.trim() === "") painted = esc(line) || " ";
            else {
                const toks = tokenise(line);
                let out = "";
                let at = 0;

                toks.forEach((t, index) => {
                    out += esc(line.slice(at, t.at));
                    at = t.at + t.text.length;

                    const raw = esc(t.text);
                    const word = t.text.toLowerCase();

                    if (index === 0) {
                        out += '<span class="code__kw' + (KEYWORDS.includes(word) ? "" : " code__kw--bad") + '">' + raw + "</span>";
                        return;
                    }

                    if (t.kind === "str") out += '<span class="code__str">' + raw + "</span>";
                    else if (t.kind === "rgx") out += '<span class="code__rgx">' + raw + "</span>";
                    else if (/^\d+$/.test(word) || DUR.test(word)) out += '<span class="code__num">' + raw + "</span>";
                    else if (/^permission:/.test(word))
                        out +=
                            '<span class="code__arg">' +
                            raw.slice(0, raw.indexOf(":") + 1) +
                            '</span><span class="code__str">' +
                            raw.slice(raw.indexOf(":") + 1) +
                            "</span>";
                    else if (/^(role|user|channel):/.test(word))
                        out +=
                            '<span class="code__arg">' +
                            raw.slice(0, raw.indexOf(":") + 1) +
                            '</span><span class="code__num">' +
                            raw.slice(raw.indexOf(":") + 1) +
                            "</span>";
                    else if (CMPS.includes(t.text)) out += '<span class="code__bin">' + raw + "</span>";
                    else out += '<span class="code__arg">' + raw + "</span>";
                });

                painted = out + esc(line.slice(at));
            }

            if (d) {
                painted +=
                    '<span class="code__lens' +
                    (d.level === "warn" ? " code__lens--warn" : "") +
                    '">' +
                    (d.level === "warn" ? "!" : "x") +
                    " " +
                    esc(d.msg) +
                    "</span>";

                if (d.help) painted += '<span class="code__help">help: ' + esc(d.help) + "</span>";

                if (d.fill) painted += '<span class="code__fill">' + esc(d.fill) + "</span>";

                painted = '<span class="' + (d.level === "warn" ? "code__warnbg" : "code__errbg") + '">' + painted + "</span>";
            }

            return painted;
        })
        .join("\n");
}
