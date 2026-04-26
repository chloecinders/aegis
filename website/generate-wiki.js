const fs = require("fs");
const path = require("path");

const SOURCE_DIR = __dirname;
const DIST_DIR = path.join(__dirname, "dist");
const WIKI_DIST_DIR = path.join(DIST_DIR, "wiki");
const WIKI_SRC_DIR = path.join(__dirname, "wiki");
const COMMANDS_SRC_DIR = path.join(__dirname, "../src/commands");
const LAYOUT_FILE = path.join(__dirname, "layout.html");

[DIST_DIR, WIKI_DIST_DIR].forEach((dir) => {
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
});

const layout = fs.readFileSync(LAYOUT_FILE, "utf8");

const LAST_UPDATED = new Date().toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric" });

function render(title, content, activePage = "", metaDescription = "") {
    return layout
        .replace(/{{TITLE}}/g, title)
        .replace(/{{CONTENT}}/g, content)
        .replace(/{{META_DESCRIPTION}}/g, metaDescription)
        .replace(/{{LAST_UPDATED}}/g, LAST_UPDATED)
        .replace(/{{ACTIVE_WIKI}}/g, activePage === "wiki" ? "active" : "")
        .replace(/{{ACTIVE_TERMS}}/g, activePage === "terms" ? "active" : "")
        .replace(/{{ACTIVE_PRIVACY}}/g, activePage === "privacy" ? "active" : "");
}

function normalizeDescription(text) {
    if (!text) return "";
    text = text.replace(/\\\r?\n\s*/g, " ");
    text = text.replace(/\\n/g, "<br>");
    text = text.replace(/\s\s+/g, " ");
    text = text
        .replace(/\*\*(.*?)\*\*/g, "<strong>$1</strong>")
        .replace(/\*(.*?)\*/g, "<em>$1</em>")
        .replace(/_(.*?)_/g, "<u>$1</u>")
        .replace(/`(.*?)`/g, "<code>$1</code>");
    return text.trim();
}

function getSyntaxDef(s) {
    let inner = "";
    let required = null;
    switch (s.type) {
        case "Consume":
        case "Reason":
            inner = `...[${s.name}]`;
            break;
        case "Filters":
            inner = "...[filters]";
            break;
        case "User":
            inner = `${s.name}: Discord User`;
            required = s.required;
            break;
        case "Member":
            inner = `${s.name}: Discord Member`;
            required = s.required;
            break;
        case "String":
            inner = `${s.name}: String`;
            required = s.required;
            break;
        case "Duration":
            inner = `${s.name}: Duration`;
            required = s.required;
            break;
        case "Number":
            inner = `${s.name}: Number`;
            required = s.required;
            break;
        case "Channel":
            inner = `${s.name}: Channel`;
            required = s.required;
            break;
        default:
            inner = s.name;
    }
    if (required !== null) return required ? `&lt;${inner}&gt;` : `[${inner}]`;
    return inner;
}

function getSyntaxExample(s) {
    switch (s.type) {
        case "Consume":
            return "Some Text";
        case "User":
            return "123456789";
        case "Member":
            return "123456789";
        case "String":
            return '"something"';
        case "Duration":
            return "15m";
        case "Reason":
            return "user broke a rule";
        case "Number":
            return "5";
        case "Channel":
            return "#some-channel";
        case "Filters":
            return "+user @aegis";
        default:
            return "value";
    }
}

function extractCommandInfo(filePath) {
    const content = fs.readFileSync(filePath, "utf8");
    const nameMatch = content.match(/fn get_name\(&self\) -> &'static str\s*{\s*"([^"]+)"/);
    const shortMatch = content.match(/fn get_short\(&self\) -> &'static str\s*{\s*"([^"]+)"/);
    const fullMatch = content.match(/fn get_full\(&self\) -> &'static str\s*{\s*"([\s\S]+?)"/);
    const categoryMatch = content.match(/fn get_category\(&self\) -> CommandCategory\s*{\s*CommandCategory::(\w+)/);
    if (!nameMatch) return null;
    const syntaxMatch = content.match(/fn get_syntax\(&self\) -> Vec<CommandSyntax>\s*{\s*vec!\[([\s\S]+?)\]/);
    let syntax = [];
    if (syntaxMatch) {
        const regex =
            /CommandSyntax::(\w+)\("([^"]+)"(?:,\s*(\w+))?\)|CommandSyntax::Reason\("([^"]+)"\)|CommandSyntax::Filters/g;
        let m;
        while ((m = regex.exec(syntaxMatch[1])) !== null) {
            if (m[1]) syntax.push({ type: m[1], name: m[2], required: m[3] === "true" });
            else if (m[4]) syntax.push({ type: "Reason", name: m[4], required: true });
            else syntax.push({ type: "Filters", name: "filters", required: false });
        }
    }
    const paramsMatch = content.match(
        /fn get_params\(&self\) -> Vec<&'static CommandParameter<'static>>\s*{\s*vec!\[([\s\S]+?)\]/,
    );
    let params = [];
    if (paramsMatch) {
        const paramBlocks = paramsMatch[1].split(/&CommandParameter\s*{/).slice(1);
        params = paramBlocks
            .map((block) => {
                const name = block.match(/name:\s*"([^"]+)"/)?.[1];
                const short = block.match(/short:\s*"([^"]+)"/)?.[1];
                const desc = block.match(/desc:\s*"([^"]+)"/)?.[1];
                return { name, short, desc: normalizeDescription(desc) };
            })
            .filter((p) => p.name);
    }
    const requiredMatch = content.match(/required:\s*vec!\[([\s\S]*?)\]/);
    const oneOfMatch = content.match(/one_of:\s*vec!\[([\s\S]*?)\]/);
    let requiredPerms = [],
        oneOfPerms = [];
    if (requiredMatch && requiredMatch[1].trim()) {
        requiredPerms = requiredMatch[1]
            .split(",")
            .map((p) => p.trim().split("::").pop())
            .filter((p) => p && !p.includes("["));
    }
    if (oneOfMatch && oneOfMatch[1].trim()) {
        oneOfPerms = oneOfMatch[1]
            .split(",")
            .map((p) => p.trim().split("::").pop())
            .filter((p) => p && !p.includes("["));
    }
    let wikiContent = "";
    const wikiMarker = content.match(/\/\/\/? WIKICONTENT/);
    if (wikiMarker) {
        const afterMarker = content.slice(wikiMarker.index + wikiMarker[0].length);
        wikiContent = afterMarker
            .split("\n")
            .map((line) => line.replace(/^\/\/\/?\s?/, "").trim())
            .join("\n")
            .trim();
    }
    return {
        name: nameMatch[1],
        short: normalizeDescription(shortMatch ? shortMatch[1] : ""),
        full: normalizeDescription(fullMatch ? fullMatch[1] : ""),
        category: categoryMatch ? categoryMatch[1] : "Misc",
        syntax,
        params,
        requiredPerms,
        oneOfPerms,
        wikiContent,
    };
}

function getAllFiles(dir, ext = ".rs", fileList = []) {
    if (!fs.existsSync(dir)) return fileList;
    const files = fs.readdirSync(dir);
    files.forEach((file) => {
        const filePath = path.join(dir, file);
        if (fs.statSync(filePath).isDirectory()) getAllFiles(filePath, ext, fileList);
        else if (file.endsWith(ext) && file !== "mod.rs") fileList.push(filePath);
    });
    return fileList;
}

const commands = getAllFiles(COMMANDS_SRC_DIR, ".rs").map(extractCommandInfo).filter(Boolean);
const categories = {};
commands.forEach((cmd) => {
    if (!categories[cmd.category]) categories[cmd.category] = [];
    categories[cmd.category].push(cmd);
});

const generalWikiFiles = getAllFiles(WIKI_SRC_DIR, ".html").map((f) => ({
    name: path.basename(f, ".html"),
    path: f,
}));

function titleCase(str) {
    return str
        .replace(/^_/, "")
        .split(/[_-]/)
        .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
        .join(" ");
}

function generateSidebar(activeItem = null) {
    let html = "<h3>General</h3><ul>";
    generalWikiFiles.forEach((page) => {
        const activeClass = activeItem === page.name ? 'class="active"' : "";
        html += `<li><a href="/wiki/${page.name}" ${activeClass}>${titleCase(page.name)}</a></li>`;
    });
    html += "</ul>";

    for (const cat in categories) {
        html += `<h3>${cat}</h3><ul>`;
        categories[cat]
            .sort((a, b) => a.name.localeCompare(b.name))
            .forEach((cmd) => {
                const activeClass = activeItem === cmd.name ? 'class="active"' : "";
                const safeName = cmd.name.replace(/[^a-z0-9]/gi, "_");
                html += `<li><a href="/wiki/${safeName}" ${activeClass}>${cmd.name}</a></li>`;
            });
        html += `</ul>`;
    }
    return html;
}

function wrapInWikiLayout(content, activeItem, sidebarHtml) {
    return `
        <div class="wiki-layout">
            <aside class="sidebar">
                <div id="sidebar-commands">${sidebarHtml}</div>
            </aside>
            <article class="wiki-content">
                ${content}
            </article>
        </div>`;
}

fs.copyFileSync(path.join(SOURCE_DIR, "styles.css"), path.join(DIST_DIR, "styles.css"));

generalWikiFiles.forEach((page) => {
    const sidebarHtml = generateSidebar(page.name);
    const content = fs.readFileSync(page.path, "utf8");
    const wikiLayout = wrapInWikiLayout(content, page.name, sidebarHtml);
    const title = titleCase(page.name);
    fs.writeFileSync(path.join(WIKI_DIST_DIR, `${page.name}.html`), render(title, wikiLayout, "wiki"));

    if (page.name === "overview") {
        fs.writeFileSync(path.join(DIST_DIR, "wiki.html"), render("Wiki", wikiLayout, "wiki"));
    }
});

commands.forEach((cmd) => {
    const sidebarHtml = generateSidebar(cmd.name);
    const safeName = cmd.name.replace(/[^a-z0-9]/gi, "_");
    const title = titleCase(cmd.name);
    const description = cmd.short || cmd.full;
    const exampleUsage = `+${cmd.name} ${cmd.syntax.map((s) => getSyntaxExample(s)).join(" ")}`.trim();

    const requiredPermsHtml =
        cmd.requiredPerms.length > 0
            ? `<span><strong>Required Permissions:</strong> ${cmd.requiredPerms.map((p) => `<span class="badge badge-permission">${p}</span>`).join(" ")}</span>`
            : "";
    const oneOfPermsHtml =
        cmd.oneOfPerms.length > 0
            ? `<span><strong>One of these Permissions:</strong> ${cmd.oneOfPerms.map((p) => `<span class="badge badge-permission">${p}</span>`).join(" ")}</span>`
            : "";
    const paramsHtml =
        cmd.params.length > 0
            ? `<h2>Parameters</h2>
        <table class="params-table">
            <thead><tr><th>Name</th><th>Flag</th><th>Description</th></tr></thead>
            <tbody>${cmd.params.map((p) => `<tr><td><code>${p.name}</code></td><td><code>-${p.short}</code></td><td>${p.desc}</td></tr>`).join("")}</tbody>
        </table>`
            : "";

    const commandContent = `
        <h1>${title}</h1>
        <p>${cmd.full || cmd.short}</p>
        <div class="command-meta">
            <span><strong>Category:</strong> <span class="badge badge-category">${cmd.category}</span></span>
            ${requiredPermsHtml}
            ${oneOfPermsHtml}
        </div>
        <h2>Usage</h2>
        <div class="syntax-box"><code>+${cmd.name} ${cmd.syntax.map((s) => getSyntaxDef(s)).join(" ")}</code></div>
        <h2>Examples</h2>
        <div class="examples-box"><code>${exampleUsage}</code></div>
        ${paramsHtml}
        ${cmd.wikiContent ? `<div class="extra-content" style="margin-top: 40px;">${cmd.wikiContent}</div>` : ""}`;

    const wikiLayout = wrapInWikiLayout(commandContent, cmd.name, sidebarHtml);
    fs.writeFileSync(path.join(WIKI_DIST_DIR, `${safeName}.html`), render(title, wikiLayout, "wiki", description));
});

const mainPages = {
    index: { title: "Home", description: "Aegis is a high-performance Discord moderation bot built with Rust." },
    terms: { title: "Terms of Service", description: "Terms of Service for the Aegis Discord moderation bot." },
    privacy: { title: "Privacy Policy", description: "Privacy Policy for the Aegis Discord moderation bot." },
};

Object.entries(mainPages).forEach(([page, meta]) => {
    const srcPath = path.join(SOURCE_DIR, `${page}.html`);
    if (fs.existsSync(srcPath)) {
        const content = fs.readFileSync(srcPath, "utf8");
        fs.writeFileSync(path.join(DIST_DIR, `${page}.html`), render(meta.title, content, page, meta.description));
    }
});

console.log(`Compiled ${generalWikiFiles.length} wiki page(s) and ${commands.length} command page(s).`);
