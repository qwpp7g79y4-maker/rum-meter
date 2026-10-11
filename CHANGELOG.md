# Changelog

All notable changes to this project are documented in this file.

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
