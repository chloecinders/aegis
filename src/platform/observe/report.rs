use std::collections::HashMap;
use std::sync::Mutex;

use tracing::{error, warn};

use crate::command::error::{Error, Severity};
use crate::domain::Snowflake;
use crate::platform::http::Http;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub label: String,
    pub count: u64,
    pub sample: Option<String>,
}

impl Summary {
    pub fn line(&self) -> String {
        let head = match self.count {
            1 => self.label.clone(),
            many => format!("{} x{many}", self.label),
        };

        match &self.sample {
            Some(sample) => format!("{head} - {sample}"),
            None => head,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Origin {
    pub command: Option<&'static str>,
    pub guild: Option<Snowflake>,
    pub channel: Option<Snowflake>,
    pub user: Option<Snowflake>,
    pub message: Option<Snowflake>,
}

impl Origin {
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();

        if let Some(command) = self.command {
            parts.push(format!("command {command}"));
        }

        for (label, id) in [
            ("guild", self.guild),
            ("channel", self.channel),
            ("user", self.user),
            ("message", self.message),
        ] {
            if let Some(id) = id {
                parts.push(format!("{label} {id}"));
            }
        }

        parts.join(", ")
    }
}

#[derive(Default)]
pub struct Counts {
    counts: Mutex<HashMap<String, Summary>>,
}

impl Counts {
    pub fn record(&self, label: String, sample: Option<String>) {
        let Ok(mut counts) = self.counts.lock() else {
            return;
        };

        let entry = counts.entry(label.clone()).or_insert(Summary {
            label,
            count: 0,
            sample: None,
        });

        entry.count += 1;

        if entry.sample.is_none() {
            entry.sample = sample;
        }
    }

    pub fn drain(&self) -> Vec<Summary> {
        let Ok(mut counts) = self.counts.lock() else {
            return Vec::new();
        };

        let mut drained: Vec<Summary> = counts.drain().map(|(_, summary)| summary).collect();

        drained.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.label.cmp(&right.label))
        });

        drained
    }
}

pub struct Reporter {
    http: Http,
    webhook: Option<String>,
    pending: Counts,
}

impl Reporter {
    pub fn new(http: Http, webhook: Option<String>) -> Self {
        Self {
            http,
            webhook,
            pending: Counts::default(),
        }
    }

    pub fn record(&self, failure: &Error, origin: Origin) {
        let severity = failure.severity();

        if severity == Severity::Expected {
            return;
        }

        let headline = failure.headline().to_string();
        let detail = failure.detail().unwrap_or_else(|| headline.clone());
        let where_from = origin.describe();

        match severity {
            Severity::Expected => {}
            Severity::Degraded => {
                warn!("{headline}: {detail}; {where_from}");
                self.pending.record(headline, Some(detail));
            }
            Severity::Bug => {
                error!("{headline}: {detail}; {where_from}");
                self.pending
                    .record(headline, Some(format!("{detail} ({where_from})")));
            }
        }
    }

    pub fn note(&self, doing: &str, detail: String) {
        warn!("{doing}: {detail}");
        self.pending.record(doing.to_string(), Some(detail));
    }

    pub async fn flush(&self) {
        let Some(webhook) = &self.webhook else {
            self.pending.drain();

            return;
        };

        let summaries = self.pending.drain();

        if summaries.is_empty() {
            return;
        }

        let content = summaries
            .iter()
            .map(Summary::line)
            .collect::<Vec<String>>()
            .join("\n");

        let sent = self
            .http
            .client()
            .post(webhook)
            .json(&serde_json::json!({ "content": content }))
            .send()
            .await;

        if let Err(err) = sent {
            warn!("could not deliver the aggregated report; err = {err:?}");
        }
    }
}
