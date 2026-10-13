use std::collections::HashSet;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

const MAX_INCREASE_ONLY_DIFF: &str = "--max-increase only works with diff";
const MAX_RUM_ONLY_COUNT: &str = "--max-rum only works with count";
const HOURS_NEEDS_PRICES: &str = "--hours-per-month only works together with --prices";
const TOP_DIFF_NEEDS_JSON: &str = "--top with diff only works together with --json";
const EXPLAIN_DIFF_NEEDS_JSON: &str = "--explain with diff only works together with --json";

/// Shown under `--help`, `count --help` and `diff --help`. Same sentences as the stderr refusals.
const USAGE_HELP: &str = "\
--max-increase only works with diff
--max-rum only works with count
--hours-per-month only works together with --prices
--top with diff only works together with --json
--explain with diff only works together with --json";

#[derive(Parser)]
// `bin_name` keeps usage lines at `rum-meter`, also as `rum-meter.exe` on Windows.
#[command(
    name = "rum-meter",
    bin_name = "rum-meter",
    version,
    about = "Estimate billable HCP Terraform RUM from local state files",
    after_help = USAGE_HELP
)]
struct Cli {
    /// Print JSON instead of text
    #[arg(long, global = true)]
    json: bool,

    /// Print one line per instance
    #[arg(long, global = true)]
    explain: bool,

    /// List at most N modules and N resource types
    #[arg(long, global = true, value_name = "N")]
    top: Option<u64>,

    /// Read tier prices from this JSON file
    #[arg(long, global = true, value_name = "FILE")]
    prices: Option<PathBuf>,

    /// Hours in a month for the cost estimate
    #[arg(long, global = true, value_name = "N")]
    hours_per_month: Option<u64>,

    /// RUM limit for count
    #[arg(long, global = true, value_name = "N")]
    max_rum: Option<u64>,

    /// RUM increase limit for diff
    #[arg(long, global = true, value_name = "N")]
    max_increase: Option<u64>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Count billable RUM in one or more state files
    #[command(after_help = USAGE_HELP)]
    Count {
        /// State file
        #[arg(value_name = "STATE", required = true, num_args = 1..)]
        states: Vec<PathBuf>,
    },
    /// Compare billable RUM in two state files
    #[command(after_help = USAGE_HELP)]
    Diff {
        /// Earlier state file
        #[arg(value_name = "OLD")]
        old: PathBuf,
        /// Later state file
        #[arg(value_name = "NEW")]
        new: PathBuf,
    },
}

fn main() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => {
            if let Some(message) = refused_option(&cli) {
                write_stderr_line(message);
                return ExitCode::from(3);
            }
            match execute(&cli) {
                Ok(RunOutcome::Success) => ExitCode::SUCCESS,
                Ok(RunOutcome::ThresholdExceeded) => ExitCode::from(1),
                Err(err) => {
                    write_stderr_line(&err.to_string());
                    ExitCode::from(2)
                }
            }
        }
        Err(err) => clap_exit(err),
    }
}

/// Help and version go to stdout and are buffered. Flush, or a full stdout
/// still looks like success. A failed write does not panic. Usage errors
/// stay exit 3.
fn clap_exit(err: clap::Error) -> ExitCode {
    let usage = err.use_stderr();
    let printed = err.print();
    let flushed = if usage {
        io::stderr().flush()
    } else {
        io::stdout().flush()
    };
    if printed.is_ok() && flushed.is_ok() {
        return if usage {
            ExitCode::from(3)
        } else {
            ExitCode::SUCCESS
        };
    }
    if usage {
        write_stderr_line("rum-meter: cannot write message to stderr");
        ExitCode::from(3)
    } else {
        write_stderr_line("rum-meter: cannot write message to stdout");
        ExitCode::from(2)
    }
}

/// One line on stderr. A full or closed stderr is ignored so the caller can
/// still return its exit code.
fn write_stderr_line(message: &str) {
    let mut stderr = io::stderr().lock();
    let _ = writeln!(stderr, "{message}");
    let _ = stderr.flush();
}

enum CliError {
    State(rum_meter::state::LoadError),
    Prices(rum_meter::cost::PriceError),
    DuplicateAddress { path: PathBuf, address: String },
    CostOverflow { path: PathBuf },
    Output,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(err) => write!(f, "{err}"),
            Self::Prices(err) => write!(f, "{err}"),
            Self::DuplicateAddress { path, address } => {
                write!(
                    f,
                    "rum-meter: {}: duplicate address {}",
                    rum_meter::rum::escape_controls(&path.display().to_string()),
                    rum_meter::rum::escape_controls(address)
                )
            }
            Self::CostOverflow { path } => {
                write!(
                    f,
                    "rum-meter: {}: monthly cost overflows",
                    rum_meter::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::Output => f.write_str("rum-meter: cannot write report to stdout"),
        }
    }
}

/// A repeated address is an input error. Checked before any report is printed.
///
/// `count` uses [`reject_repeated_address`], which also refuses the same
/// address when a later file repeats it. `diff` checks each file on its own,
/// because the old and new states are compared, not added.
fn reject_duplicate(path: &Path, verdicts: &[rum_meter::rum::Verdict]) -> Result<(), CliError> {
    match rum_meter::rum::find_duplicate_address(verdicts) {
        Some(address) => Err(CliError::DuplicateAddress {
            path: path.to_path_buf(),
            address: address.to_string(),
        }),
        None => Ok(()),
    }
}

/// The same address in one file, or in a later file passed to `count`, is an
/// input error. The path is the file that repeats the address.
fn reject_repeated_address(
    files: &[(&Path, Vec<rum_meter::rum::Verdict>)],
) -> Result<(), CliError> {
    let mut seen = HashSet::new();
    for (path, verdicts) in files {
        for verdict in verdicts {
            if !seen.insert(verdict.address.as_str()) {
                return Err(CliError::DuplicateAddress {
                    path: path.to_path_buf(),
                    address: verdict.address.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Result of a report that was printed. A threshold breach is exit 1, not an error line.
enum RunOutcome {
    Success,
    ThresholdExceeded,
}

/// A flag that would not change the report. Exit 3, before any file is opened.
///
/// The first match wins: `--max-increase` on count, `--max-rum` on diff,
/// `--hours-per-month` without `--prices`, then on diff without `--json`
/// first `--top` and then `--explain`.
fn refused_option(cli: &Cli) -> Option<&'static str> {
    let is_diff = matches!(&cli.command, Commands::Diff { .. });
    if !is_diff && cli.max_increase.is_some() {
        return Some(MAX_INCREASE_ONLY_DIFF);
    }
    if is_diff && cli.max_rum.is_some() {
        return Some(MAX_RUM_ONLY_COUNT);
    }
    if cli.hours_per_month.is_some() && cli.prices.is_none() {
        return Some(HOURS_NEEDS_PRICES);
    }
    if is_diff && cli.top.is_some() && !cli.json {
        return Some(TOP_DIFF_NEEDS_JSON);
    }
    if is_diff && cli.explain && !cli.json {
        return Some(EXPLAIN_DIFF_NEEDS_JSON);
    }
    None
}

/// Load every state before printing, so an input error stays on stderr only.
///
/// `--max-rum` limits the summed `count`. `--max-increase` limits the `diff`
/// increase (new RUM minus old RUM). An equal value is within the limit.
/// The report is printed either way. Flags that do not apply are refused
/// by [`refused_option`] before this function reads a file.
fn execute(cli: &Cli) -> Result<RunOutcome, CliError> {
    match &cli.command {
        Commands::Count { states } => {
            let prices = match &cli.prices {
                Some(path) => Some((
                    path.as_path(),
                    rum_meter::cost::load_prices(path).map_err(CliError::Prices)?,
                )),
                None => None,
            };
            let mut loaded = Vec::with_capacity(states.len());
            for path in states {
                loaded.push((
                    path.as_path(),
                    rum_meter::state::load_state(path).map_err(CliError::State)?,
                ));
            }
            let counted: Vec<_> = loaded
                .iter()
                .map(|(path, state)| (*path, rum_meter::rum::classify(state)))
                .collect();
            reject_repeated_address(&counted)?;
            let files: Vec<_> = counted
                .iter()
                .map(|(path, verdicts)| rum_meter::render::CountedFile { path, verdicts })
                .collect();
            let rum: usize = files
                .iter()
                .map(|file| rum_meter::rum::total_rum(file.verdicts))
                .sum();
            let costs = match &prices {
                Some((path, prices)) => rum_meter::cost::monthly_costs(
                    u64::try_from(rum).unwrap_or(u64::MAX),
                    prices,
                    cli.hours_per_month
                        .unwrap_or(rum_meter::cost::DEFAULT_HOURS_PER_MONTH),
                )
                .map_err(|_| CliError::CostOverflow {
                    path: path.to_path_buf(),
                })?,
                None => Vec::new(),
            };
            let report = if cli.json {
                rum_meter::render::render_count_json(&files, cli.explain, cli.top, &costs)
            } else {
                rum_meter::render::render_count_text(&files, cli.explain, cli.top, &costs)
            };
            write_report(&report)?;
            Ok(outcome(
                cli.max_rum
                    .is_some_and(|max_rum| count_exceeds(rum, max_rum)),
            ))
        }
        Commands::Diff { old, new } => {
            let prices = match &cli.prices {
                Some(path) => Some((
                    path.as_path(),
                    rum_meter::cost::load_prices(path).map_err(CliError::Prices)?,
                )),
                None => None,
            };
            let old_state = rum_meter::state::load_state(old).map_err(CliError::State)?;
            let new_state = rum_meter::state::load_state(new).map_err(CliError::State)?;
            let old_verdicts = rum_meter::rum::classify(&old_state);
            let new_verdicts = rum_meter::rum::classify(&new_state);
            reject_duplicate(old, &old_verdicts)?;
            reject_duplicate(new, &new_verdicts)?;
            let diff = rum_meter::diff::diff_verdicts(&old_verdicts, &new_verdicts);
            let hours = cli
                .hours_per_month
                .unwrap_or(rum_meter::cost::DEFAULT_HOURS_PER_MONTH);
            let old_rum = u64::try_from(diff.old_rum).unwrap_or(u64::MAX);
            let new_rum = u64::try_from(diff.new_rum).unwrap_or(u64::MAX);
            let cost_deltas = match &prices {
                Some((path, prices)) => {
                    rum_meter::diff::cost_deltas(old_rum, new_rum, prices, hours).map_err(|_| {
                        CliError::CostOverflow {
                            path: path.to_path_buf(),
                        }
                    })?
                }
                None => Vec::new(),
            };
            let report = if cli.json {
                let costs = match &prices {
                    Some((path, prices)) => rum_meter::cost::monthly_costs(new_rum, prices, hours)
                        .map_err(|_| CliError::CostOverflow {
                            path: path.to_path_buf(),
                        })?,
                    None => Vec::new(),
                };
                rum_meter::render::render_diff_json(
                    &rum_meter::render::DiffJsonInput {
                        old,
                        new,
                        old_verdicts: &old_verdicts,
                        new_verdicts: &new_verdicts,
                        diff: &diff,
                        costs: &costs,
                        cost_deltas: &cost_deltas,
                    },
                    cli.explain,
                    cli.top,
                )
            } else {
                rum_meter::render::render_diff_text(old, new, &diff, &cost_deltas)
            };
            write_report(&report)?;
            Ok(outcome(cli.max_increase.is_some_and(|max_increase| {
                increase_exceeds(diff.rum_delta(), max_increase)
            })))
        }
    }
}

/// Flush explicitly so a buffered write failure cannot turn success into a panic.
fn write_report(report: &str) -> Result<(), CliError> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(report.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|_| CliError::Output)
}

/// `--max-rum` compared with the summed count. A total that does not fit in `u64` is over every limit.
fn count_exceeds(rum: usize, max_rum: u64) -> bool {
    match u64::try_from(rum) {
        Ok(rum) => rum > max_rum,
        Err(_) => true,
    }
}

/// `--max-increase` compared with new RUM minus old RUM. A decrease is within every limit.
fn increase_exceeds(delta: i64, max_increase: u64) -> bool {
    match u64::try_from(delta) {
        Ok(delta) => delta > max_increase,
        Err(_) => false,
    }
}

fn outcome(exceeded: bool) -> RunOutcome {
    if exceeded {
        RunOutcome::ThresholdExceeded
    } else {
        RunOutcome::Success
    }
}
