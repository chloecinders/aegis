use regex::Regex;
use sqlx::query;

use crate::{SQL, database::ActionType, utils::consume_pgsql_error};

pub struct RuleCache {
    ocr: Vec<Rule>
}

impl RuleCache {
    pub fn new() -> Self {
        Self {
            ocr: Vec::new()
        }
    }

    pub fn insert_ocr(&mut self, rule: Rule) {
        self.ocr.push(rule);
    }

    pub fn has_ocr_rules(&self, guild_id: u64) -> bool {
        for rule in &self.ocr {
            if rule.guild_id == guild_id {
                return true;
            }
        }

        false
    }

    pub fn matches(&self, guild_id: u64, input: String) -> Option<&Rule> {
        for rule in &self.ocr {
            if rule.guild_id == guild_id && rule.matches(&input) {
                return Some(rule);
            }
        }

        return None;
    }

    pub async fn populate_from_db(&mut self) {
        let res = match query!("
            SELECT
                id,
                name,
                guild_id,
                type,
                rule,
                is_regex,
                created_at,
                reason,
                punishment_type as \"punishment_type!: ActionType\",
                duration,
                silent,
                day_clear_amount
            FROM automod_rules;
        ").fetch_all(&*SQL).await {
            Ok(d) => d,
            Err(err) => {
                consume_pgsql_error("POPULATE RULE CACHE".into(), err);
                return;
            },
        };

        res.into_iter().for_each(|record| {
            let punish = match record.punishment_type {
                ActionType::Softban => Punishment::Softban {
                    reason: record.reason,
                    day_clear_amount: record.day_clear_amount.unwrap_or(0) as u8,
                    silent: record.silent.unwrap_or(false)
                },
                _ => Punishment::Warn {
                    reason: record.reason,
                    silent: record.silent.unwrap_or(false)
                }
            };

            let rule = Rule {
                name: record.name,
                id: record.id,
                rule: record.rule,
                is_regex: record.is_regex,
                guild_id: record.guild_id as u64,
                punishment: punish,
            };

            match record.r#type.as_str() {
                "ocr" => self.ocr.push(rule),
                _ => {}
            };
        });
    }
}

#[derive(Clone, Debug)]
pub enum Punishment {
    Warn { reason: String, silent: bool },
    Softban { reason: String, day_clear_amount: u8, silent: bool },
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub name: String,
    pub id: String,
    pub rule: String,
    pub is_regex: bool,
    pub guild_id: u64,
    pub punishment: Punishment,
}

impl Rule {
    pub fn matches(&self, input: &str) -> bool {
        if self.is_regex {
            let Ok(regex) = Regex::new(&self.rule) else { return false };

            regex.is_match(input)
        } else {
            input.contains(&self.rule)
        }
    }
}
