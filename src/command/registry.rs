use std::collections::HashMap;
use std::iter::once;

use crate::command::args::Field;
use crate::command::cx::Cx;
use crate::command::error::Result;
use crate::command::slash::cx::SlashCx;
use crate::command::slash::{Parameter, Reply, Slash};
use crate::command::stream::Stream;
use crate::command::{Boxed, Category, Command, Meta, Response};

pub type Execute = for<'a> fn(&'a mut Cx, &'a mut Stream) -> Boxed<'a, Result<Response>>;
pub type Rehearse = for<'a> fn(&'a mut Cx, &'a mut Stream) -> Boxed<'a, Result<serde_json::Value>>;
pub type Answer = for<'a> fn(&'a mut SlashCx) -> Boxed<'a, Result<Reply>>;

#[derive(Clone, Copy)]
pub struct Text {
    pub fields: &'static [Field],
    pub execute: Execute,
    pub rehearse: Rehearse,
}

impl Text {
    pub fn syntax(&self) -> String {
        self.fields
            .iter()
            .filter(|field| !field.is_flag())
            .map(Field::syntax)
            .collect::<Vec<String>>()
            .join(" ")
    }

    pub fn example(&self) -> String {
        self.fields
            .iter()
            .filter_map(Field::example)
            .collect::<Vec<&str>>()
            .join(" ")
    }

    pub fn parameters(&self) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter(|field| field.is_flag())
    }
}

#[derive(Clone, Copy)]
pub struct ChatInput {
    pub parameters: &'static [Parameter],
    pub execute: Answer,
}

#[derive(Clone, Copy)]
pub struct Entry {
    pub meta: Meta,
    pub text: Option<Text>,
    pub slash: Option<ChatInput>,
}

fn dispatch<'a, C: Command>(cx: &'a mut Cx, stream: &'a mut Stream) -> Boxed<'a, Result<Response>> {
    Box::pin(async move {
        let parsed = C::parse(cx, stream).await?;

        cx.trace("parse");
        cx.remember(C::META.name, parsed.snapshot());

        parsed.run(cx).await
    })
}

fn rehearse<'a, C: Command>(
    cx: &'a mut Cx,
    stream: &'a mut Stream,
) -> Boxed<'a, Result<serde_json::Value>> {
    Box::pin(async move { Ok(C::parse(cx, stream).await?.snapshot()) })
}

fn answer<'a, S: Slash>(cx: &'a mut SlashCx) -> Boxed<'a, Result<Reply>> {
    Box::pin(async move { S::parse(cx)?.run(cx).await })
}

#[derive(Default)]
pub struct Registry {
    entries: Vec<Entry>,
    index: HashMap<&'static str, usize>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    fn entry(&mut self, meta: Meta) -> &mut Entry {
        if let Some(position) = self.index.get(meta.name).copied() {
            let existing = &mut self.entries[position];

            assert!(
                existing.meta == meta,
                "{} is registered twice with different meta",
                meta.name
            );

            return existing;
        }

        let position = self.entries.len();

        self.entries.push(Entry {
            meta,
            text: None,
            slash: None,
        });

        for name in once(&meta.name).chain(meta.aliases) {
            assert!(
                self.index.insert(name, position).is_none(),
                "two commands with the same name ({name})"
            );
        }

        &mut self.entries[position]
    }

    pub fn add<C: Command>(&mut self) {
        let entry = self.entry(C::META);

        assert!(
            entry.text.is_none(),
            "two commands with the same name ({})",
            C::META.name
        );

        entry.text = Some(Text {
            fields: C::FIELDS,
            execute: dispatch::<C>,
            rehearse: rehearse::<C>,
        });
    }

    pub fn add_slash<S: Slash>(&mut self) {
        const {
            assert!(
                !S::META.short.is_empty() && S::META.short.len() <= 100,
                "a slash command description is 1 to 100 characters"
            );
        }

        let entry = self.entry(S::META);

        assert!(
            entry.slash.is_none(),
            "two commands with the same name ({})",
            S::META.name
        );

        entry.slash = Some(ChatInput {
            parameters: S::PARAMETERS,
            execute: answer::<S>,
        });
    }

    pub fn find(&self, name: &str) -> Option<&Entry> {
        if let Some(position) = self.index.get(name) {
            return self.entries.get(*position);
        }

        let lowered = name.to_lowercase();

        self.index
            .get(lowered.as_str())
            .and_then(|position| self.entries.get(*position))
    }

    pub fn all(&self) -> &[Entry] {
        &self.entries
    }

    pub fn in_category(&self, category: Category) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(move |entry| entry.meta.category == category)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[macro_export]
macro_rules! register {
    ($registry:expr, $($command:ty),+ $(,)?) => {
        $($registry.add::<$command>();)+
    };
}

#[macro_export]
macro_rules! register_slash {
    ($registry:expr, $($command:ty),+ $(,)?) => {
        $($registry.add_slash::<$command>();)+
    };
}
