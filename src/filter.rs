use crate::error::{LoglensError, Result};
use crate::model::{FieldFilter, LogLevel, LogRecord};
use crate::parser::parse_timestamp;
use chrono::{DateTime, FixedOffset};
use regex::Regex;

#[derive(Debug)]
pub struct FilterOptions {
    pub level: Option<LogLevel>,
    pub min_level: Option<LogLevel>,
    pub keyword: Option<String>,
    pub pattern: Option<Regex>,
    pub from: Option<DateTime<FixedOffset>>,
    pub to: Option<DateTime<FixedOffset>>,
    pub fields: Vec<FieldFilter>,
}

impl FilterOptions {
    pub fn new(
        level: Option<LogLevel>,
        min_level: Option<LogLevel>,
        keyword: Option<String>,
        regex: Option<String>,
        from: Option<String>,
        to: Option<String>,
        fields: Vec<String>,
    ) -> Result<Self> {
        let fields = fields
            .into_iter()
            .map(|value| {
                FieldFilter::parse(&value).ok_or(LoglensError::InvalidFieldFilter { value })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            level,
            min_level,
            keyword: keyword.map(|value| value.to_ascii_lowercase()),
            pattern: regex.map(|value| Regex::new(&value)).transpose()?,
            from: from.map(|value| parse_timestamp(&value)).transpose()?,
            to: to.map(|value| parse_timestamp(&value)).transpose()?,
            fields,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.level.is_none()
            && self.min_level.is_none()
            && self.keyword.is_none()
            && self.pattern.is_none()
            && self.from.is_none()
            && self.to.is_none()
            && self.fields.is_empty()
    }

    pub fn matches(&self, record: &LogRecord) -> bool {
        if self.level.is_some_and(|level| record.level != Some(level)) {
            return false;
        }

        if let Some(min_level) = self.min_level {
            let Some(record_level) = record.level else {
                return false;
            };
            if record_level.rank() < min_level.rank() {
                return false;
            }
        }

        if !self
            .fields
            .iter()
            .all(|filter| field_matches(record, filter))
        {
            return false;
        }

        if let Some(keyword) = &self.keyword {
            let haystack = record_search_text(record).to_ascii_lowercase();
            if !haystack.contains(keyword) {
                return false;
            }
        }

        if let Some(pattern) = &self.pattern {
            let haystack = record_search_text(record);
            if !pattern.is_match(&haystack) {
                return false;
            }
        }

        if let Some(from) = self.from
            && record.timestamp.is_none_or(|timestamp| timestamp < from)
        {
            return false;
        }

        if let Some(to) = self.to
            && record.timestamp.is_none_or(|timestamp| timestamp > to)
        {
            return false;
        }

        true
    }
}

pub fn apply_filters(records: Vec<LogRecord>, options: &FilterOptions) -> Vec<LogRecord> {
    if options.is_empty() {
        return records;
    }

    records
        .into_iter()
        .filter(|record| options.matches(record))
        .collect()
}

fn record_search_text(record: &LogRecord) -> String {
    let mut text = record.message.clone();
    text.push(' ');
    text.push_str(&record.source);
    for (key, value) in &record.fields {
        text.push(' ');
        text.push_str(key);
        text.push('=');
        text.push_str(value);
    }
    text
}

fn field_matches(record: &LogRecord, filter: &FieldFilter) -> bool {
    record
        .fields
        .get(&filter.key)
        .is_some_and(|value| value == &filter.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LogRecord;

    #[test]
    fn filters_by_level_and_keyword() {
        let mut record = LogRecord::new("app.log", "database timeout");
        record.level = Some(LogLevel::Error);
        let options = FilterOptions::new(
            Some(LogLevel::Error),
            None,
            Some("timeout".to_owned()),
            None,
            None,
            None,
            Vec::new(),
        )
        .expect("filter should build");

        assert!(options.matches(&record));
    }

    #[test]
    fn filters_by_regex() {
        let record = LogRecord::new("app.log", "request took 340ms");
        let options = FilterOptions::new(
            None,
            None,
            None,
            Some(r"\d+ms".to_owned()),
            None,
            None,
            Vec::new(),
        )
        .expect("filter should build");

        assert!(options.matches(&record));
    }

    #[test]
    fn filters_by_min_level() {
        let mut info = LogRecord::new("app.log", "ok");
        info.level = Some(LogLevel::Info);
        let mut error = LogRecord::new("app.log", "failed");
        error.level = Some(LogLevel::Error);
        let options = FilterOptions::new(
            None,
            Some(LogLevel::Warn),
            None,
            None,
            None,
            None,
            Vec::new(),
        )
        .expect("filter should build");

        assert!(!options.matches(&info));
        assert!(options.matches(&error));
    }

    #[test]
    fn filters_by_field_key_value() {
        let mut record = LogRecord::new("app.log", "request complete");
        record.fields.insert("user".to_owned(), "alice".to_owned());
        record.fields.insert("service".to_owned(), "api".to_owned());
        let options = FilterOptions::new(
            None,
            None,
            None,
            None,
            None,
            None,
            vec!["user=alice".to_owned(), "service=api".to_owned()],
        )
        .expect("filter should build");

        assert!(options.matches(&record));
    }

    #[test]
    fn rejects_invalid_field_filter() {
        let err = FilterOptions::new(None, None, None, None, None, None, vec!["user".to_owned()])
            .expect_err("field filter should require key=value");

        assert!(err.to_string().contains("key=value"));
    }
}
