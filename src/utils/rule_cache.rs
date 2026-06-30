use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::Value;

use crate::{SQL, database::ActionType, utils::consume_pgsql_error};

const IMAGE_HASH_CACHE_MAX: usize = 10000;
const OCR_RESULT_CACHE_MAX: usize = 1000;

/// Stores the OCR output for a single message attachment, plus whether it matched a rule.
#[derive(Clone, Debug)]
pub struct OcrDebugEntry {
    /// The raw OCR text extracted from the image.
    pub text: String,
    /// If the text matched a rule: (rule name, rule id, matched pattern, is_regex).
    pub matched: Option<(String, String, String, bool)>,
}

/// A small bounded FIFO cache that maps message_id → Vec of per-attachment debug entries.
pub struct OcrResultCache {
    entries: std::collections::HashMap<u64, Vec<OcrDebugEntry>>,
    order: std::collections::VecDeque<u64>,
}

impl OcrResultCache {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
        }
    }

    pub fn insert(&mut self, message_id: u64, entries: Vec<OcrDebugEntry>) {
        if !self.entries.contains_key(&message_id) {
            if self.entries.len() >= OCR_RESULT_CACHE_MAX {
                if let Some(old) = self.order.pop_front() {
                    self.entries.remove(&old);
                }
            }
            self.order.push_back(message_id);
        }
        self.entries.insert(message_id, entries);
    }

    pub fn get(&self, message_id: u64) -> Option<&Vec<OcrDebugEntry>> {
        self.entries.get(&message_id)
    }
}

#[derive(Clone, Debug)]
pub struct OcrPattern {
    pub pattern: String,
    pub is_regex: bool,
}

pub struct ImageHashCache {
    entries: HashMap<(u64, String), Option<String>>,
    order: VecDeque<(u64, String)>,
}

impl ImageHashCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&self, guild_id: u64, image_hash: &str) -> Option<&Option<String>> {
        self.entries.get(&(guild_id, image_hash.to_string()))
    }

    pub fn insert(&mut self, guild_id: u64, image_hash: String, rule_id: Option<String>) {
        let key = (guild_id, image_hash);
        if !self.entries.contains_key(&key) {
            if self.entries.len() >= IMAGE_HASH_CACHE_MAX {
                if let Some(old_key) = self.order.pop_front() {
                    self.entries.remove(&old_key);
                }
            }
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, rule_id);
    }

    pub fn invalidate_rule(&mut self, rule_id: &str) {
        let to_remove: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, v)| v.as_deref() == Some(rule_id))
            .map(|(k, _)| k.clone())
            .collect();

        for key in &to_remove {
            self.entries.remove(key);
        }

        self.order.retain(|k| self.entries.contains_key(k));
    }

    fn byte_footprint(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.entries.capacity()
                * (std::mem::size_of::<(u64, String)>() + std::mem::size_of::<Option<String>>())
            + self
                .entries
                .iter()
                .map(|(k, v)| k.1.capacity() + v.as_ref().map(|s| s.capacity()).unwrap_or(0))
                .sum::<usize>()
            + self.order.capacity() * std::mem::size_of::<(u64, String)>()
    }
}

pub struct RuleCache {
    ocr: Vec<Rule>,
    recent_triggers: HashMap<(String, u64), Instant>,
    pub image_hash_cache: ImageHashCache,
}

impl RuleCache {
    pub fn new() -> Self {
        Self {
            ocr: Vec::new(),
            recent_triggers: HashMap::new(),
            image_hash_cache: ImageHashCache::new(),
        }
    }

    pub fn check_debounce(&mut self, rule_id: String, user_id: u64) -> bool {
        let key = (rule_id, user_id);
        if let Some(last_triggered) = self.recent_triggers.get(&key) {
            if last_triggered.elapsed() < Duration::from_secs(15) {
                return false;
            }
        }

        if self.recent_triggers.len() > 1000 {
            self.recent_triggers
                .retain(|_, v| v.elapsed() < Duration::from_secs(15));
        }

        self.recent_triggers.insert(key, Instant::now());
        true
    }

    pub fn insert_ocr(&mut self, rule: Rule) {
        self.ocr.push(rule);
    }

    pub fn remove(&mut self, id: &str) {
        self.ocr.retain(|r| r.id != id);
        self.image_hash_cache.invalidate_rule(id);
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Rule> {
        self.ocr.iter().find(|r| r.id == id)
    }

    pub fn add_pattern_to_rule(&mut self, rule_id: &str, pattern: OcrPattern) {
        if let Some(rule) = self.ocr.iter_mut().find(|r| r.id == rule_id) {
            rule.patterns.push(pattern);
        }
    }

    pub fn has_ocr_rules(&self, guild_id: u64) -> bool {
        self.ocr.iter().any(|r| r.guild_id == guild_id)
    }

    pub fn matches(&self, guild_id: u64, input: String) -> Option<(Rule, OcrPattern)> {
        for rule in &self.ocr {
            if rule.guild_id == guild_id {
                if let Some(pat) = rule.matches_detail(&input) {
                    return Some((rule.clone(), pat.clone()));
                }
            }
        }
        None
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
                patterns,
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

            let patterns: Vec<OcrPattern> = if let Some(Value::Array(arr)) =
                record.get::<Option<Value>, _>("patterns")
            {
                arr.into_iter()
                    .filter_map(|v| {
                        let pattern = v.get("pattern")?.as_str()?.to_string();
                        let is_regex = v.get("is_regex").and_then(|b| b.as_bool()).unwrap_or(false);
                        Some(OcrPattern { pattern, is_regex })
                    })
                    .collect()
            } else {
                vec![OcrPattern {
                    pattern: record.get("rule"),
                    is_regex: record.get("is_regex"),
                }]
            };

            if patterns.is_empty() {
                return;
            }

            let rule = Rule {
                name: record.get("name"),
                id: record.get("id"),
                patterns,
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
            + self.recent_triggers.capacity() * std::mem::size_of::<((String, u64), Instant)>()
            + self.image_hash_cache.byte_footprint()
    }
}

pub async fn db_check_image_hash(guild_id: u64, image_hash: &str) -> Option<String> {
    let result = sqlx::query(
        "SELECT rule_id FROM ocr_image_hashes \
         WHERE guild_id = $1 AND image_hash = $2 LIMIT 1",
    )
    .bind(guild_id as i64)
    .bind(image_hash)
    .fetch_optional(&*SQL)
    .await;

    match result {
        Ok(Some(row)) => {
            use sqlx::Row;
            Some(row.get("rule_id"))
        }
        _ => None,
    }
}

pub async fn db_record_image_hash(
    guild_id: u64,
    image_hash: &str,
    rule_id: &str,
    is_regex: bool,
    matched_pattern: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO ocr_image_hashes \
             (image_hash, rule_id, guild_id, is_regex, matched_pattern) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (image_hash, rule_id) DO NOTHING",
    )
    .bind(image_hash)
    .bind(rule_id)
    .bind(guild_id as i64)
    .bind(is_regex)
    .bind(matched_pattern)
    .execute(&*SQL)
    .await;
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
    pub patterns: Vec<OcrPattern>,
    pub guild_id: u64,
    pub punishment: Punishment,
}

impl Rule {
    pub fn matches_detail<'a>(&'a self, input: &str) -> Option<&'a OcrPattern> {
        for pat in &self.patterns {
            let matched = if pat.is_regex {
                Regex::new(&pat.pattern)
                    .map(|re| re.is_match(input))
                    .unwrap_or(false)
            } else {
                fuzzy_substring_match(&pat.pattern, input, 0.95)
            };

            if matched {
                return Some(pat);
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn matches(&self, input: &str) -> bool {
        self.matches_detail(input).is_some()
    }

    pub fn byte_footprint(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.name.capacity()
            + self.id.capacity()
            + self
                .patterns
                .iter()
                .map(|p| std::mem::size_of::<OcrPattern>() + p.pattern.capacity())
                .sum::<usize>()
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
