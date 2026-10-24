# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Fixed

- `diff` matches objects on module, mode, type, name, index key and deposed key, not on the printed address. A resource named `a[0]` and resource `a` at index 0 are now one removal and one addition instead of no change.
- A price file with two tiers of the same name is exit 2, `duplicate tier name`, and prints no cost.
- Text output now escapes U+2028 (LINE SEPARATOR) and U+2029 (PARAGRAPH SEPARATOR) the same way it escapes other invisible or bidirectional format characters. An address, file path, module name, type or price label carrying one of these could otherwise print a raw character that several renderers treat as a line break, faking an extra line in a report.
- `docs/reference.md` and `docs/reference.nl.md` said `--version` shows `rum-meter 0.1.0`; it has shown `rum-meter 0.1.1` since that release. The text now matches, and a test pins it to the crate version.

### Added

- A state path `-` reads stdin when stdin is a pipe or a regular file. A terminal or a device on stdin is refused. The quick start includes a block that needs no file from this repository.
- A `cargo-fuzz` harness (`fuzz/`, excluded from the published crate) with one target per stage that reads untrusted input: state parsing, price-tier parsing, cost calculation, `diff`, and rendering. Each asserts no panic, that JSON output always parses, that amounts are never negative, NaN or infinite, and that fields the tool drops (`attributes`, `sensitive_attributes`, `private`, `outputs`) never reach a report. A cheap nightly build-only CI job keeps the targets buildable.
- `proptest` property tests that run under plain `cargo test`: the state and price parsers never panic on arbitrary bytes, the monthly cost matches an independent rational-arithmetic reference, and rendering the same verdicts twice is stable with JSON that always parses.
- Regression tests for newline-only files, a lone CR, an escaped NUL byte, Unicode spaces and lookalike letters, a duplicate JSON key outside a price tier, deeply nested JSON, and the numbers `1e309`, `-0` and an out-of-range integer.

## [0.1.1] - 2026-10-14

### Fixed

- A price is read as the exact decimal from the file. 1 RUM at 0.004999999999999999999 for 1 hour is 0.00. 1 RUM at 1.005 for 1 hour stays 1.01.
- A state whose root value is a JSON array is refused with exit 2, `state is not an object`.
- `count` refuses a repeated address across the files it is given, including the same file twice.
- A path that is swapped for a FIFO between the check and the read no longer makes the tool wait; the opened file itself is checked.
- A write error on stdout or stderr is a clean error instead of a panic. `--version` does not exit 0 when its output cannot be written.

## [0.1.0] - 2026-10-11

### Added

- `count`: billable RUM in one or more local Terraform state files of format version 4.
- `diff`: added and removed counting addresses, rule changes, and the RUM delta between two states.
- `--explain`: one line per instance with the rule code, and ` tainted` when the object is tainted.
- `--top`: the first N modules and resource types; the rest as one `other` row.
- `--json`: one JSON object with `schema_version` 1.
- `--prices` and `--hours-per-month`: a monthly estimate from a price file you supply. The default is 730 hours.
- `--max-rum` and `--max-increase`: exit 1 when the threshold is exceeded. The report is still printed.
- A flag that would not change the report exits 3 with a fixed message, before any file is read.
- Costs use decimal arithmetic and are rounded half away from zero to whole cents, so 1 RUM at 1.005 for 1 hour is 1.01. 0 hours is always 0.00.
- Text reports and error messages escape control characters and invisible or bidirectional format characters in addresses, file paths and price labels. JSON keeps the original strings.
- A report that cannot be written to stdout exits 2 with `rum-meter: cannot write report to stdout`.
- Only regular files are read. A UTF-8 BOM is skipped, in state files and price files. UTF-16 and UTF-32 are refused, with or without a BOM.
- Release binaries for Linux x86_64 (glibc and musl), Windows x86_64 and macOS (Intel and Apple Silicon), each with a SHA-256 checksum.
