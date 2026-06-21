use crate::model::{GroupBy, LogLevel, LogRecord};
use chrono::{Datelike, Timelike};
use serde::Serialize;
use std::collections::BTreeMap;

pub trait Aggregator<T> {
    type Output;

    fn add(&mut self, item: &T);
    fn finish(self) -> Self::Output;
}

#[derive(Debug, Default)]
pub struct CountByLevel {
    counts: BTreeMap<String, usize>,
}

impl Aggregator<LogRecord> for CountByLevel {
    type Output = BTreeMap<String, usize>;

    fn add(&mut self, item: &LogRecord) {
        let key = item
            .level
            .map(|level| level.to_string())
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        *self.counts.entry(key).or_default() += 1;
    }

    fn finish(self) -> Self::Output {
        self.counts
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Summary {
    pub total: usize,
    pub level_counts: BTreeMap<String, usize>,
    pub grouped_counts: BTreeMap<String, usize>,
    pub source_counts: BTreeMap<String, usize>,
    pub top_sources: Vec<(String, usize)>,
    pub top_field_name: Option<String>,
    pub top_field_values: Vec<(String, usize)>,
}

pub fn summarize(
    records: &[LogRecord],
    group_by: GroupBy,
    group_field: Option<&str>,
    top_n: usize,
    top_field: Option<&str>,
) -> Summary {
    let mut level_counter = CountByLevel::default();
    for record in records {
        level_counter.add(record);
    }

    Summary {
        total: records.len(),
        level_counts: level_counter.finish(),
        grouped_counts: group_records(records, group_by, group_field),
        source_counts: group_records(records, GroupBy::Source, None),
        top_sources: top_values(records.iter().map(|record| record.source.as_str()), top_n),
        top_field_name: top_field.map(str::to_owned),
        top_field_values: top_field
            .map(|field| top_field_values_including_missing(records, field, top_n))
            .unwrap_or_default(),
    }
}

pub fn group_records(
    records: &[LogRecord],
    group_by: GroupBy,
    field_name: Option<&str>,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();

    for record in records {
        let key = match group_by {
            GroupBy::Level => record
                .level
                .map(|level| level.to_string())
                .unwrap_or_else(|| "UNKNOWN".to_owned()),
            GroupBy::Source => record.source.clone(),
            GroupBy::Hour => record
                .timestamp
                .map(|timestamp| {
                    format!(
                        "{:04}-{:02}-{:02} {:02}:00",
                        timestamp.year(),
                        timestamp.month(),
                        timestamp.day(),
                        timestamp.hour()
                    )
                })
                .unwrap_or_else(|| "UNKNOWN_TIME".to_owned()),
            GroupBy::Field => match field_name {
                Some(field_name) => record
                    .fields
                    .get(field_name)
                    .cloned()
                    .unwrap_or_else(|| "MISSING".to_owned()),
                None => "MISSING_FIELD_NAME".to_owned(),
            },
        };
        *counts.entry(key).or_default() += 1;
    }

    counts
}

pub fn top_field_values(records: &[LogRecord], field: &str, limit: usize) -> Vec<(String, usize)> {
    top_values(
        records
            .iter()
            .filter_map(|record| record.fields.get(field).map(String::as_str)),
        limit,
    )
}

pub fn top_field_values_including_missing(
    records: &[LogRecord],
    field: &str,
    limit: usize,
) -> Vec<(String, usize)> {
    top_values(
        records
            .iter()
            .map(|record| record.fields.get(field).map_or("MISSING", String::as_str)),
        limit,
    )
}

fn top_values<'a>(values: impl Iterator<Item = &'a str>, limit: usize) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(value.to_owned()).or_default() += 1;
    }

    let mut ordered = counts.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    ordered.truncate(limit);
    ordered
}

#[allow(dead_code)]
fn _keep_loglevel_order_visible(_: LogLevel) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LogRecord;

    #[test]
    fn summarizes_level_counts() {
        let mut first = LogRecord::new("a.log", "ok");
        first.level = Some(LogLevel::Info);
        let mut second = LogRecord::new("a.log", "bad");
        second.level = Some(LogLevel::Error);
        let records = vec![first, second];

        let summary = summarize(&records, GroupBy::Level, None, 3, None);

        assert_eq!(summary.total, 2);
        assert_eq!(summary.level_counts["INFO"], 1);
        assert_eq!(summary.level_counts["ERROR"], 1);
    }

    #[test]
    fn returns_top_field_values() {
        let mut first = LogRecord::new("a.log", "one");
        first.fields.insert("user".to_owned(), "alice".to_owned());
        let mut second = LogRecord::new("b.log", "two");
        second.fields.insert("user".to_owned(), "alice".to_owned());

        assert_eq!(
            top_field_values(&[first, second], "user", 1),
            vec![("alice".to_owned(), 2)]
        );
    }

    #[test]
    fn groups_by_requested_field_with_missing_bucket() {
        let mut first = LogRecord::new("a.log", "one");
        first.fields.insert("user".to_owned(), "alice".to_owned());
        let second = LogRecord::new("b.log", "two");

        let counts = group_records(&[first, second], GroupBy::Field, Some("user"));

        assert_eq!(counts["alice"], 1);
        assert_eq!(counts["MISSING"], 1);
    }

    #[test]
    fn top_field_values_include_missing_and_sort_by_name_on_tie() {
        let mut first = LogRecord::new("a.log", "one");
        first.fields.insert("user".to_owned(), "bob".to_owned());
        let mut second = LogRecord::new("b.log", "two");
        second.fields.insert("user".to_owned(), "alice".to_owned());
        let third = LogRecord::new("c.log", "three");

        assert_eq!(
            top_field_values_including_missing(&[first, second, third], "user", 3),
            vec![
                ("MISSING".to_owned(), 1),
                ("alice".to_owned(), 1),
                ("bob".to_owned(), 1)
            ]
        );
    }
}
