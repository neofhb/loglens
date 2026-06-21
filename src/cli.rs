use crate::error::{LoglensError, Result};
use crate::filter::{FilterOptions, apply_filters};
use crate::input::load_dataset;
use crate::model::{GroupBy, InputFormat, LogLevel};
use crate::report::{print_issues, print_records, print_summary, write_report};
use crate::stats::summarize;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "loglens")]
#[command(about = "Analyze local log, JSONL, and CSV files")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Scan(InputArgs),
    Filter(FilterCommand),
    Stats(StatsCommand),
    Report(ReportCommand),
}

#[derive(Debug, Args)]
struct InputArgs {
    path: PathBuf,

    #[arg(long, value_enum, default_value_t = InputFormat::Auto)]
    format: InputFormat,

    #[arg(long)]
    strict: bool,
}

#[derive(Debug, Args)]
struct FilterCommand {
    #[command(flatten)]
    input: InputArgs,

    #[command(flatten)]
    filters: FilterArgs,

    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Args)]
struct StatsCommand {
    #[command(flatten)]
    input: InputArgs,

    #[arg(long, value_enum, default_value_t = GroupBy::Level)]
    group_by: GroupBy,

    #[arg(long)]
    field_name: Option<String>,

    #[arg(long)]
    top_field: Option<String>,

    #[arg(long, default_value_t = 5)]
    top: usize,
}

#[derive(Debug, Args)]
struct ReportCommand {
    #[command(flatten)]
    input: InputArgs,

    #[command(flatten)]
    filters: FilterArgs,

    #[arg(long, value_enum, default_value_t = GroupBy::Level)]
    group_by: GroupBy,

    #[arg(long)]
    field_name: Option<String>,

    #[arg(long)]
    top_field: Option<String>,

    #[arg(long, default_value_t = 5)]
    top: usize,

    #[arg(long, default_value_t = 20)]
    sample_limit: usize,

    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct FilterArgs {
    #[arg(long, value_enum)]
    level: Option<LogLevel>,

    #[arg(long, value_enum)]
    min_level: Option<LogLevel>,

    #[arg(long)]
    keyword: Option<String>,

    #[arg(long)]
    regex: Option<String>,

    #[arg(long)]
    from: Option<String>,

    #[arg(long)]
    to: Option<String>,

    #[arg(long)]
    field: Vec<String>,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Scan(args) => {
            let dataset = load_dataset(&args.path, args.format, args.strict)?;
            println!("Loaded {} records", dataset.records.len());
            println!("Parse issues: {}", dataset.issue_count());
            print_records(&dataset.records, 10);
            print_issues(&dataset.issues, 5);
        }
        Command::Filter(command) => {
            let filters = command.filters.build()?;
            let dataset = load_dataset(
                &command.input.path,
                command.input.format,
                command.input.strict,
            )?;
            let records = apply_filters(dataset.records, &filters);
            println!("Matched {} records", records.len());
            print_records(&records, command.limit);
            print_issues(&dataset.issues, 5);
        }
        Command::Stats(command) => {
            let group_field = group_field_name(command.group_by, command.field_name.as_deref())?;
            let dataset = load_dataset(
                &command.input.path,
                command.input.format,
                command.input.strict,
            )?;
            let summary = summarize(
                &dataset.records,
                command.group_by,
                group_field,
                command.top,
                command.top_field.as_deref(),
            );
            print_summary(&summary);
            print_issues(&dataset.issues, 5);
        }
        Command::Report(command) => {
            let filters = command.filters.build()?;
            let group_field = group_field_name(command.group_by, command.field_name.as_deref())?;
            let dataset = load_dataset(
                &command.input.path,
                command.input.format,
                command.input.strict,
            )?;
            let records = apply_filters(dataset.records, &filters);
            let summary = summarize(
                &records,
                command.group_by,
                group_field,
                command.top,
                command.top_field.as_deref(),
            );
            write_report(
                &command.output,
                &summary,
                &records,
                &dataset.issues,
                command.sample_limit,
            )?;
            println!(
                "Wrote report for {} records to {}",
                records.len(),
                command.output.display()
            );
        }
    }

    Ok(())
}

impl FilterArgs {
    fn build(self) -> Result<FilterOptions> {
        FilterOptions::new(
            self.level,
            self.min_level,
            self.keyword,
            self.regex,
            self.from,
            self.to,
            self.field,
        )
    }
}

fn group_field_name(group_by: GroupBy, field_name: Option<&str>) -> Result<Option<&str>> {
    if group_by == GroupBy::Field && field_name.is_none() {
        return Err(LoglensError::MissingFieldName);
    }
    Ok(field_name)
}
