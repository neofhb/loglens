use chrono::{DateTime, FixedOffset};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InputFormat {
    Auto,
    PlainLog,
    Jsonl,
    Csv,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum GroupBy {
    Level,
    Source,
    Hour,
    Field,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" | "information" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" => Some(Self::Error),
            "fatal" | "critical" => Some(Self::Fatal),
            _ => None,
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Trace => 0,
            Self::Debug => 1,
            Self::Info => 2,
            Self::Warn => 3,
            Self::Error => 4,
            Self::Fatal => 5,
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Fatal => "FATAL",
        };
        f.write_str(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogRecord {
    pub timestamp: Option<DateTime<FixedOffset>>,
    pub level: Option<LogLevel>,
    pub source: String,
    pub message: String,
    pub fields: BTreeMap<String, String>,
}

impl LogRecord {
    pub fn new(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            timestamp: None,
            level: None,
            source: source.into(),
            message: message.into(),
            fields: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParseIssue {
    pub source: String,
    pub line: usize,
    pub reason: String,
    pub raw: String,
}

impl ParseIssue {
    pub fn new(
        source: impl Into<String>,
        line: usize,
        reason: impl Into<String>,
        raw: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            line,
            reason: reason.into(),
            raw: raw.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogDataset {
    pub records: Vec<LogRecord>,
    pub issues: Vec<ParseIssue>,
}

impl LogDataset {
    pub fn new(records: Vec<LogRecord>, issues: Vec<ParseIssue>) -> Self {
        Self { records, issues }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn extend(&mut self, other: LogDataset) {
        self.records.extend(other.records);
        self.issues.extend(other.issues);
    }

    pub fn issue_count(&self) -> usize {
        self.issues.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldFilter {
    pub key: String,
    pub value: String,
}

impl FieldFilter {
    pub fn parse(value: &str) -> Option<Self> {
        let (key, field_value) = value.split_once('=')?;
        let key = key.trim();
        if key.is_empty() {
            return None;
        }

        Some(Self {
            key: key.to_owned(),
            value: field_value.trim().to_owned(),
        })
    }
}
