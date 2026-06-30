document.addEventListener("DOMContentLoaded", async () => {
    const app = document.getElementById("app");
    if (!app) return;

    let data = window.TRANSCRIPT_DATA;

    const dataEl = document.getElementById("transcript-data");
    if (!data && dataEl) {
        try {
            data = JSON.parse(dataEl.textContent);
        } catch (e) {
            console.error("Failed to parse transcript data:", e);
        }
    }

    if (!data) {
        const parts = window.location.pathname.split("/").filter(Boolean);
        if (parts[0] === "transcript" && parts.length >= 3) {
            app.innerHTML =
                '<div style="color: #dbdee1; padding: 40px; font-family: Inter, sans-serif; text-align: center;">Loading transcript...</div>';
            try {
                const res = await fetch(`/api/transcript/${parts[1]}/${parts[2]}`);
                if (res.ok) {
                    data = await res.json();
                    app.innerHTML = "";
                } else {
                    app.innerHTML =
                        '<div style="color: #f23f43; padding: 40px; font-family: Inter, sans-serif; text-align: center;">Transcript not found or expired.</div>';
                    return;
                }
            } catch (err) {
                app.innerHTML =
                    '<div style="color: #f23f43; padding: 40px; font-family: Inter, sans-serif; text-align: center;">Failed to load transcript data.</div>';
                return;
            }
        }
    }

    if (!data) return;

    const container = document.createElement("div");
    container.className = "discord-transcript-container";

    const header = document.createElement("div");
    header.className = "discord-header";
    header.innerHTML = `
        <div class="discord-header-title">
            <span>${escapeHtml(data.meta.channel_name)}</span>
        </div>
        <div class="discord-header-meta">
            <span><strong>Server:</strong> ${escapeHtml(data.meta.guild_name)}</span>
            <span><strong>Moderator:</strong> ${escapeHtml(data.meta.moderator)}</span>
            <span><strong>Purged:</strong> ${data.meta.count} messages</span>
            <span><strong>Timestamp:</strong> ${escapeHtml(data.meta.timestamp)}</span>
        </div>
    `;
    container.appendChild(header);

    const messagesDiv = document.createElement("div");
    messagesDiv.className = "discord-messages";

    let lastAuthorId = null;

    data.messages.forEach((m) => {
        const isContinuation = lastAuthorId === m.author.id;
        lastAuthorId = m.author.id;

        const row = document.createElement("div");
        row.className = isContinuation ? "discord-message continuation" : "discord-message";

        if (isContinuation) {
            const gutter = document.createElement("div");
            gutter.className = "discord-avatar-gutter";
            gutter.textContent = m.timestamp_gutter;
            row.appendChild(gutter);
        } else {
            const avatarDiv = document.createElement("div");
            avatarDiv.className = "discord-avatar";
            if (m.author.avatar_url) {
                avatarDiv.innerHTML = `<img src="${escapeHtml(m.author.avatar_url)}" alt="avatar" />`;
            } else {
                avatarDiv.style.backgroundColor = "#5865f2";
                avatarDiv.textContent = (m.author.name[0] || "?").toUpperCase();
            }
            row.appendChild(avatarDiv);
        }

        const contentDiv = document.createElement("div");
        contentDiv.className = "discord-msg-content";

        if (!isContinuation) {
            const headerDiv = document.createElement("div");
            headerDiv.className = "discord-msg-header";

            let headerHtml = `<span class="discord-author">${escapeHtml(m.author.name)}</span>`;
            if (m.author.is_bot) {
                headerHtml += `<span class="discord-bot-badge">BOT</span>`;
            }
            headerHtml += `
                <span class="discord-user-id">${m.author.id}</span>
                <span class="discord-timestamp">${escapeHtml(m.timestamp_header)}</span>
                <span class="discord-msg-id">ID: ${m.id}</span>
            `;
            headerDiv.innerHTML = headerHtml;
            contentDiv.appendChild(headerDiv);
        }

        const bodyContainer = document.createElement("div");
        bodyContainer.className = isContinuation ? "discord-continuation-row" : "discord-message-row";

        const bodyDiv = document.createElement("div");
        bodyDiv.className = "discord-body";
        bodyDiv.innerHTML = formatContent(m.content);

        if (m.attachments && m.attachments.length > 0) {
            m.attachments.forEach((att) => {
                const attDiv = document.createElement("div");
                attDiv.className = "discord-attachment";
                if (att.is_image) {
                    attDiv.innerHTML = `<img src="${escapeHtml(att.url)}" style="max-width: 100%; border-radius: 4px;" />`;
                } else {
                    attDiv.innerHTML = `<a href="${escapeHtml(att.url)}" target="_blank" style="color: #00a8fc;">📎 ${escapeHtml(att.filename)}</a>`;
                }
                bodyDiv.appendChild(attDiv);
            });
        }

        if (m.embeds && m.embeds.length > 0) {
            m.embeds.forEach((e) => {
                const embedDiv = document.createElement("div");
                embedDiv.className = "discord-embed";
                embedDiv.style.borderLeftColor = e.border_color || "#202225";

                if (e.author) {
                    const authDiv = document.createElement("div");
                    authDiv.className = "discord-embed-author";
                    let authHtml = "";
                    if (e.author.icon_url) {
                        authHtml += `<img src="${escapeHtml(e.author.icon_url)}" class="discord-embed-author-icon" />`;
                    }
                    authHtml += `<span>${escapeHtml(e.author.name)}</span>`;
                    authDiv.innerHTML = authHtml;
                    embedDiv.appendChild(authDiv);
                }

                if (e.title) {
                    if (e.url) {
                        const titleA = document.createElement("a");
                        titleA.className = "discord-embed-title";
                        titleA.href = e.url;
                        titleA.target = "_blank";
                        titleA.textContent = e.title;
                        embedDiv.appendChild(titleA);
                    } else {
                        const titleDiv = document.createElement("div");
                        titleDiv.className = "discord-embed-title";
                        titleDiv.textContent = e.title;
                        embedDiv.appendChild(titleDiv);
                    }
                }

                if (e.description) {
                    const descDiv = document.createElement("div");
                    descDiv.className = "discord-embed-desc";
                    descDiv.innerHTML = formatContent(e.description);
                    embedDiv.appendChild(descDiv);
                }

                if (e.fields && e.fields.length > 0) {
                    const fieldsDiv = document.createElement("div");
                    fieldsDiv.className = "discord-embed-fields";
                    e.fields.forEach((f) => {
                        const fieldDiv = document.createElement("div");
                        fieldDiv.className = f.inline ? "discord-embed-field inline" : "discord-embed-field";
                        fieldDiv.innerHTML = `
                            <div class="discord-embed-field-name">${escapeHtml(f.name)}</div>
                            <div class="discord-embed-field-value">${formatContent(f.value)}</div>
                        `;
                        fieldsDiv.appendChild(fieldDiv);
                    });
                    embedDiv.appendChild(fieldsDiv);
                }

                if (e.footer) {
                    const footDiv = document.createElement("div");
                    footDiv.className = "discord-embed-footer";
                    let footHtml = "";
                    if (e.footer.icon_url) {
                        footHtml += `<img src="${escapeHtml(e.footer.icon_url)}" class="discord-embed-footer-icon" />`;
                    }
                    footHtml += `<span>${escapeHtml(e.footer.text)}</span>`;
                    footDiv.innerHTML = footHtml;
                    embedDiv.appendChild(footDiv);
                }

                bodyDiv.appendChild(embedDiv);
            });
        }

        bodyContainer.appendChild(bodyDiv);

        if (isContinuation) {
            const idSpan = document.createElement("span");
            idSpan.className = "discord-msg-id";
            idSpan.textContent = `ID: ${m.id}`;
            bodyContainer.appendChild(idSpan);
        }

        contentDiv.appendChild(bodyContainer);
        row.appendChild(contentDiv);
        messagesDiv.appendChild(row);
    });

    container.appendChild(messagesDiv);
    app.appendChild(container);
});

function escapeHtml(str) {
    if (!str) return "";
    return String(str)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
}

function formatContent(str) {
    if (!str) return "";
    let text = escapeHtml(str);

    const codeBlocks = [];
    text = text.replace(/(?:\r?\n)?```(?:[a-zA-Z0-9_-]*\r?\n)?([\s\S]*?)```(?:\r?\n)?/g, (match, code) => {
        code = code.replace(/^\r?\n/, "").replace(/\r?\n$/, "");
        const id = `%%%CODE_BLOCK_${codeBlocks.length}%%%`;
        codeBlocks.push(`<pre class="discord-code-block"><code>${code}</code></pre>`);
        return id;
    });

    const inlineCodes = [];
    text = text.replace(/`([^`]+)`/g, (match, code) => {
        const id = `%%%INLINE_CODE_${inlineCodes.length}%%%`;
        inlineCodes.push(`<code class="discord-inline-code">${code}</code>`);
        return id;
    });

    if (text.includes("&gt;&gt;&gt; ")) {
        const idx = text.indexOf("&gt;&gt;&gt; ");
        const before = text.substring(0, idx);
        const quote = text.substring(idx + 13);
        text = before + `<blockquote class="discord-blockquote">${quote}</blockquote>`;
    } else {
        const lines = text.split("\n");
        let inQuote = false;
        let resultLines = [];
        for (let i = 0; i < lines.length; i++) {
            if (lines[i].startsWith("&gt; ")) {
                if (!inQuote) {
                    resultLines.push('<blockquote class="discord-blockquote">');
                    inQuote = true;
                }
                resultLines.push(lines[i].substring(5));
            } else {
                if (inQuote) {
                    resultLines.push("</blockquote>");
                    inQuote = false;
                }
                resultLines.push(lines[i]);
            }
        }
        if (inQuote) {
            resultLines.push("</blockquote>");
        }
        text = resultLines.join("\n");
    }

    text = text.replace(/^-# (.*$)/gm, '<div class="discord-subtext">$1</div>');

    text = text.replace(
        /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
        '<a href="$2" target="_blank" class="discord-link">$1</a>',
    );
    text = text.replace(/(^|\s)(https?:\/\/[^\s<]+)/g, (match, prefix, url) => {
        return `${prefix}<a href="${url}" target="_blank" class="discord-link">${url}</a>`;
    });

    text = text.replace(/^### (.*$)/gm, '<h3 class="discord-header">$1</h3>');
    text = text.replace(/^## (.*$)/gm, '<h2 class="discord-header">$1</h2>');
    text = text.replace(/^# (.*$)/gm, '<h1 class="discord-header">$1</h1>');

    text = text.replace(
        /\|\|([\s\S]+?)\|\|/g,
        '<span class="discord-spoiler" onclick="this.classList.toggle(\'revealed\')">$1</span>',
    );
    text = text.replace(/\*\*([\s\S]+?)\*\*/g, "<strong>$1</strong>");
    text = text.replace(/__([\s\S]+?)__/g, "<u>$1</u>");
    text = text.replace(/\*([^\*]+?)\*/g, "<em>$1</em>");
    text = text.replace(/\b_([^_]+?)_\b/g, "<em>$1</em>");
    text = text.replace(/~~([\s\S]+?)~~/g, "<del>$1</del>");

    text = text.replace(/\n/g, "<br />");

    inlineCodes.forEach((html, i) => {
        text = text.replace(`%%%INLINE_CODE_${i}%%%`, html);
    });
    codeBlocks.forEach((html, i) => {
        text = text.replace(`%%%CODE_BLOCK_${i}%%%`, html);
    });

    return text;
}
