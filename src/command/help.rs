use serenity::all::Permissions;

use crate::command::registry::{Entry, Registry, Text};
use crate::command::{CATEGORIES, Category};
use crate::platform::ui::embed::{Embed, codeblock};
use crate::platform::ui::tone::Tone;

pub struct Page {
    pub category: Category,
    pub body: String,
}

fn section(registry: &Registry, category: Category, developer: bool) -> Option<Page> {
    let lines: Vec<String> = registry
        .in_category(category)
        .filter(|entry| entry.text.is_some())
        .filter(|entry| (developer || !entry.meta.developer) && !entry.meta.hidden)
        .map(|entry| format!("`{}` - {}", entry.meta.name, entry.meta.short))
        .collect();

    if lines.is_empty() {
        return None;
    }

    Some(Page {
        category,
        body: lines.join("\n"),
    })
}

pub fn pages(registry: &Registry, developer: bool) -> Vec<Page> {
    CATEGORIES
        .iter()
        .filter_map(|category| section(registry, *category, developer))
        .collect()
}

pub fn sheet(pages: &[Page], at: usize, prefix: &str) -> Embed {
    let Some(page) = pages.get(at) else {
        return Embed::new("COMMANDS")
            .footnote("There is nothing here you can run")
            .tone(Tone::Info);
    };

    Embed::new("COMMANDS")
        .subtitle(format!("Category: {}", page.category))
        .body(page.body.clone())
        .footnote(format!(
            "`{prefix}help <command>` for detail | page {} of {}",
            at + 1,
            pages.len()
        ))
        .tone(Tone::Info)
}

pub fn permission_names(permissions: Permissions) -> Vec<String> {
    permissions
        .get_permission_names()
        .into_iter()
        .map(|name| name.to_uppercase().replace(' ', "_"))
        .collect()
}

fn invocation(entry: &Entry, prefix: &str, rest: String) -> String {
    match rest.is_empty() {
        true => format!("{prefix}{}", entry.meta.name),
        false => format!("{prefix}{} {rest}", entry.meta.name),
    }
}

pub fn detail(entry: &Entry, text: &Text, prefix: &str) -> Embed {
    let mut body = entry.meta.full.replace("/p/", prefix);

    let parameters: Vec<String> = text
        .parameters()
        .map(|flag| {
            format!(
                "`{}` -> {}",
                flag.switch(),
                flag.desc.replace("/p/", prefix)
            )
        })
        .collect();

    if !parameters.is_empty() {
        body.push_str(&format!(
            "\n\nOptional Parameters:\n{}",
            parameters.join("\n")
        ));
    }

    body.push_str(&format!(
        "\n\nSyntax:\n{}\nExample:\n{}",
        codeblock(&invocation(entry, prefix, text.syntax())),
        codeblock(&invocation(entry, prefix, text.example()))
    ));

    let required = permission_names(entry.meta.user);
    let alternatives = permission_names(entry.meta.one_of);

    let requirement = match (required.is_empty(), alternatives.is_empty()) {
        (true, true) => None,
        (false, true) => Some(required.join(" && ")),
        (true, false) => Some(alternatives.join(" || ")),
        (false, false) => Some(format!(
            "{} && ({})",
            required.join(" && "),
            alternatives.join(" || ")
        )),
    };

    if let Some(permissions) = requirement {
        body.push_str(&format!("\nRequired Permissions:\n`{permissions}`"));
    }

    Embed::new(entry.meta.name.to_uppercase())
        .maybe_subtitle(match entry.meta.aliases.is_empty() {
            true => None,
            false => Some(format!("Aliases: {}", entry.meta.aliases.join(", "))),
        })
        .body(body)
        .tone(Tone::Info)
}
