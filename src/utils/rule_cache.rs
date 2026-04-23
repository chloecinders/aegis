use regex::Regex;

use crate::{SQL, database::ActionType, utils::consume_pgsql_error};

pub struct RuleCache {
    ocr: Vec<Rule>,
}

impl RuleCache {
    pub fn new() -> Self {
        Self { ocr: Vec::new() }
    }

    pub fn insert_ocr(&mut self, rule: Rule) {
        self.ocr.push(rule);
    }

    pub fn remove(&mut self, id: &str) {
        self.ocr.retain(|r| r.id != id);
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
        let res = match sqlx::query(
            "
            SELECT
                id,
                name,
                guild_id,
                type,
                rule,
                is_regex,
                created_at,
                reason,
                punishment_type,
                duration,
                silent,
                day_clear_amount,
                log_channel_id
            FROM automod_rules;
        ",
        )
        .fetch_all(&*SQL)
        .await
        {
            Ok(d) => d,
            Err(err) => {
                consume_pgsql_error("POPULATE RULE CACHE".into(), err);
                return;
            }
        };

        use sqlx::Row;
        res.into_iter().for_each(|record| {
            let punishment_type: ActionType = record.get("punishment_type");
            let punish = match punishment_type {
                ActionType::Softban => Punishment::Softban {
                    reason: record.get("reason"),
                    day_clear_amount: record
                        .get::<Option<i16>, _>("day_clear_amount")
                        .unwrap_or(0) as u8,
                    silent: record.get::<Option<bool>, _>("silent").unwrap_or(false),
                },
                ActionType::Ban => Punishment::Ban {
                    reason: record.get("reason"),
                    day_clear_amount: record
                        .get::<Option<i16>, _>("day_clear_amount")
                        .unwrap_or(0) as u8,
                    duration: record.get::<Option<i64>, _>("duration").unwrap_or(0) as u64,
                    silent: record.get::<Option<bool>, _>("silent").unwrap_or(false),
                },
                ActionType::Kick => Punishment::Kick {
                    reason: record.get("reason"),
                    silent: record.get::<Option<bool>, _>("silent").unwrap_or(false),
                },
                ActionType::Mute => Punishment::Mute {
                    reason: record.get("reason"),
                    duration: record.get::<Option<i64>, _>("duration").unwrap_or(0) as u64,
                    silent: record.get::<Option<bool>, _>("silent").unwrap_or(false),
                },
                ActionType::Log => Punishment::Log {
                    reason: record.get("reason"),
                    channel_id: record.get::<Option<i64>, _>("log_channel_id").unwrap_or(0) as u64,
                },
                _ => Punishment::Warn {
                    reason: record.get("reason"),
                    silent: record.get::<Option<bool>, _>("silent").unwrap_or(false),
                },
            };

            let rule = Rule {
                name: record.get("name"),
                id: record.get("id"),
                rule: record.get("rule"),
                is_regex: record.get("is_regex"),
                guild_id: record.get::<i64, _>("guild_id") as u64,
                punishment: punish,
            };

            match record.get::<String, _>("type").as_str() {
                "ocr" => self.ocr.push(rule),
                _ => {}
            };
        });
    }

    pub fn byte_footprint(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.ocr.capacity() * std::mem::size_of::<Rule>()
            + self
                .ocr
                .iter()
                .map(|r| r.byte_footprint() - std::mem::size_of::<Rule>())
                .sum::<usize>()
    }
}

#[derive(Clone, Debug)]
pub enum Punishment {
    Warn {
        reason: String,
        silent: bool,
    },
    Kick {
        reason: String,
        silent: bool,
    },
    Ban {
        reason: String,
        day_clear_amount: u8,
        duration: u64,
        silent: bool,
    },
    Softban {
        reason: String,
        day_clear_amount: u8,
        silent: bool,
    },
    Mute {
        reason: String,
        duration: u64,
        silent: bool,
    },
    Log {
        reason: String,
        channel_id: u64,
    },
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
            let Ok(regex) = Regex::new(&self.rule) else {
                return false;
            };

            regex.is_match(input)
        } else {
            fuzzy_substring_match(&self.rule, input, 0.95)
        }
    }

    pub fn byte_footprint(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.name.capacity()
            + self.id.capacity()
            + self.rule.capacity()
            + match &self.punishment {
                Punishment::Warn { reason, .. }
                | Punishment::Kick { reason, .. }
                | Punishment::Ban { reason, .. }
                | Punishment::Softban { reason, .. }
                | Punishment::Mute { reason, .. }
                | Punishment::Log { reason, .. } => reason.capacity(),
            }
    }
}

fn fuzzy_substring_match(rule: &str, input: &str, threshold: f64) -> bool {
    if input.contains(rule) {
        return true;
    }
    let rule_lower = rule.to_lowercase();
    let input_lower = input.to_lowercase();
    if input_lower.contains(&rule_lower) {
        return true;
    }

    let m = rule_lower.chars().count();
    let n = input_lower.chars().count();
    if m == 0 {
        return true;
    }
    if n == 0 || m > n {
        return false;
    }

    let max_errors = ((1.0 - threshold) * m as f64).ceil() as usize;

    let r_chars: Vec<char> = rule_lower.chars().collect();
    let i_chars: Vec<char> = input_lower.chars().collect();

    let mut dp = vec![0; n + 1];

    for i in 1..=m {
        let mut prev = dp[0];
        dp[0] = i;
        for j in 1..=n {
            let temp = dp[j];
            let cost = if r_chars[i - 1] == i_chars[j - 1] {
                0
            } else {
                1
            };
            dp[j] = std::cmp::min(std::cmp::min(dp[j] + 1, dp[j - 1] + 1), prev + cost);
            prev = temp;
        }
    }

    let min_dist = *dp.iter().skip(1).min().unwrap_or(&m);
    min_dist <= max_errors
}
