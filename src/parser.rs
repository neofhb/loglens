use crate::error::{LoglensError, Result};
use crate::model::{InputFormat, LogDataset, LogLevel, LogRecord, ParseIssue};
use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone};
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

pub trait RecordParser {
    fn parse_reader<R: BufRead>(&self, reader: R, source: &str) -> Result<LogDataset>;
}

pub struct PlainLogParser;
pub struct JsonlParser;
pub struct CsvLogParser;

impl RecordParser for PlainLogParser {
    fn parse_reader<R: BufRead>(&self, reader: R, source: &str) -> Result<LogDataset> {
        let bracketed = Regex::new(
            r"^\s*(?:\[(?P<ts>[^\]]+)\]\s*)?(?:\[(?P<level>[A-Za-z]+)\]\s*)?(?P<msg>.*)$",
        )?;
        let simple = Regex::new(
            r"^\s*(?:(?P<ts>\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:Z|[+-]\d{2}:\d{2})?)\s+)?(?P<level>TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|CRITICAL)\s+(?P<msg>.*)$",
        )?;

        let mut records = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(parse_plain_line(&line, source, &simple, &bracketed));
        }
        Ok(LogDataset::new(records, Vec::new()))
    }
}

impl RecordParser for JsonlParser {
    fn parse_reader<R: BufRead>(&self, reader: R, source: &str) -> Result<LogDataset> {
        let mut records = Vec::new();
        let mut issues = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(&line) {
                Ok(value) => records.push(json_value_to_record(value, source)),
                Err(error) => issues.push(ParseIssue::new(
                    source,
                    index + 1,
                    format!("invalid JSON: {error}"),
                    line,
                )),
            }
        }
        Ok(LogDataset::new(records, issues))
    }
}

impl RecordParser for CsvLogParser {
    fn parse_reader<R: BufRead>(&self, reader: R, source: &str) -> Result<LogDataset> {
        let mut csv_reader = csv::Reader::from_reader(reader);
        let headers = csv_reader.headers()?.clone();
        let mut records = Vec::new();
        let mut issues = Vec::new();

        for row in csv_reader.records() {
            let row = match row {
                Ok(row) => row,
                Err(error) => {
                    let line = error
                        .position()
                        .map(|position| position.line())
                        .unwrap_or(0);
                    issues.push(ParseIssue::new(
                        source,
                        line as usize,
                        format!("invalid CSV row: {error}"),
                        String::new(),
                    ));
                    continue;
                }
            };
            let mut fields = BTreeMap::new();
            let mut timestamp = None;
            let mut level = None;
            let mut message = String::new();

            for (header, value) in headers.iter().zip(row.iter()) {
                let key = header.trim().to_ascii_lowercase();
                match key.as_str() {
                    "timestamp" | "time" | "ts" => timestamp = parse_timestamp(value).ok(),
                    "level" | "severity" => level = LogLevel::parse(value),
                    "message" | "msg" | "event" => message = value.to_owned(),
                    _ => {
                        fields.insert(header.to_owned(), value.to_owned());
                    }
                }
            }

            if message.is_empty() {
                message = row.iter().collect::<Vec<_>>().join(" ");
            }

            records.push(LogRecord {
                timestamp,
                level,
                source: source.to_owned(),
                message,
                fields,
            });
        }

        Ok(LogDataset::new(records, issues))
    }
}

pub fn parse_records<R: BufRead>(
    reader: R,
    source: &str,
    format: InputFormat,
) -> Result<LogDataset> {
    match format {
        InputFormat::Auto => PlainLogParser.parse_reader(reader, source),
        InputFormat::PlainLog => PlainLogParser.parse_reader(reader, source),
        InputFormat::Jsonl => JsonlParser.parse_reader(reader, source),
        InputFormat::Csv => CsvLogParser.parse_reader(reader, source),
    }
}

pub fn detect_format(path: &Path, requested: InputFormat) -> Result<InputFormat> {
    if requested != InputFormat::Auto {
        return Ok(requested);
    }

    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jsonl" | "ndjson") => Ok(InputFormat::Jsonl),
        Some("csv") => Ok(InputFormat::Csv),
        Some("log" | "txt") | None => Ok(InputFormat::PlainLog),
        _ => Err(LoglensError::UnsupportedFormat {
            path: path.to_path_buf(),
        }),
    }
}

pub fn parse_timestamp(value: &str) -> Result<DateTime<FixedOffset>> {
    let trimmed = value.trim().trim_matches(['[', ']']);
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(timestamp);
    }

    let normalized = trimmed.replace('T', " ");
    let naive = NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S").map_err(|_| {
        LoglensError::InvalidTimestamp {
            value: value.to_owned(),
        }
    })?;
    let offset = FixedOffset::east_opt(8 * 3600).expect("valid fixed offset");
    offset
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| LoglensError::InvalidTimestamp {
            value: value.to_owned(),
        })
}

fn parse_plain_line(line: &str, source: &str, simple: &Regex, bracketed: &Regex) -> LogRecord {
    if let Some(captures) = simple.captures(line) {
        let mut record = LogRecord::new(source, captures["msg"].trim());
        record.timestamp = captures
            .name("ts")
            .and_then(|value| parse_timestamp(value.as_str()).ok());
        record.level = captures
            .name("level")
            .and_then(|value| LogLevel::parse(value.as_str()));
        return record;
    }

    if let Some(captures) = bracketed.captures(line) {
        let mut record = LogRecord::new(source, captures["msg"].trim());
        record.timestamp = captures
            .name("ts")
            .and_then(|value| parse_timestamp(value.as_str()).ok());
        record.level = captures
            .name("level")
            .and_then(|value| LogLevel::parse(value.as_str()));
        return record;
    }

    LogRecord::new(source, line.trim())
}

fn json_value_to_record(value: Value, source: &str) -> LogRecord {
    let Some(object) = value.as_object() else {
        return LogRecord::new(source, value.to_string());
    };

    let mut fields = BTreeMap::new();
    let mut timestamp = None;
    let mut level = None;
    let mut message = None;

    for (key, value) in object {
        let normalized = key.to_ascii_lowercase();
        let text = value_to_text(value);
        match normalized.as_str() {
            "timestamp" | "time" | "ts" => timestamp = parse_timestamp(&text).ok(),
            "level" | "severity" => level = LogLevel::parse(&text),
            "message" | "msg" | "event" => message = Some(text),
            _ => {
                fields.insert(key.clone(), text);
            }
        }
    }

    LogRecord {
        timestamp,
        level,
        source: source.to_owned(),
        message: message.unwrap_or_else(|| serde_json::to_string(object).unwrap_or_default()),
        fields,
    }
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_plain_log_lines() {
        let input =
            "2026-06-20 10:00:00 INFO service started\n[2026-06-20 10:01:00] [ERROR] timeout\n";
        let records = PlainLogParser
            .parse_reader(Cursor::new(input), "app.log")
            .expect("plain log should parse")
            .records;

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].level, Some(LogLevel::Info));
        assert_eq!(records[1].level, Some(LogLevel::Error));
        assert_eq!(records[1].message, "timeout");
    }

    #[test]
    fn parses_jsonl_records() {
        let input = r#"{"timestamp":"2026-06-20T10:00:00+08:00","level":"warn","message":"slow","service":"api"}"#;
        let records = JsonlParser
            .parse_reader(Cursor::new(input), "app.jsonl")
            .expect("jsonl should parse")
            .records;

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].level, Some(LogLevel::Warn));
        assert_eq!(records[0].fields["service"], "api");
    }

    #[test]
    fn parses_csv_records() {
        let input = "timestamp,level,message,user\n2026-06-20 10:00:00,error,failed login,alice\n";
        let records = CsvLogParser
            .parse_reader(Cursor::new(input), "app.csv")
            .expect("csv should parse")
            .records;

        assert_eq!(records[0].level, Some(LogLevel::Error));
        assert_eq!(records[0].fields["user"], "alice");
    }

    #[test]
    fn invalid_jsonl_collects_issue() {
        let dataset = JsonlParser
            .parse_reader(Cursor::new("{bad json}"), "app.jsonl")
            .expect("invalid json should become issue");

        assert!(dataset.records.is_empty());
        assert_eq!(dataset.issues[0].line, 1);
        assert!(dataset.issues[0].reason.contains("invalid JSON"));
    }

    #[test]
    fn jsonl_keeps_valid_records_around_bad_lines() {
        let input = "{\"level\":\"info\",\"message\":\"ok\"}\n{bad json}\n{\"level\":\"error\",\"message\":\"bad\"}";
        let dataset = JsonlParser
            .parse_reader(Cursor::new(input), "app.jsonl")
            .expect("jsonl should parse partially");

        assert_eq!(dataset.records.len(), 2);
        assert_eq!(dataset.issues.len(), 1);
        assert_eq!(dataset.records[1].level, Some(LogLevel::Error));
    }
}
