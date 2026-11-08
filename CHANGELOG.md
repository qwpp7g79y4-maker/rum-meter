# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Added

- `--markdown` and `--html` print a Markdown or a self-contained HTML report for `count` and `diff`, next to text and `--json`. The three format flags exclude each other (exit 3). Snapshot tests in `tests/report_formats.rs`.

### Changed

- README, README.nl, `QUALITY.md` and `QUALITY.nl.md` state the measured test count (250).

## [1.0.0] - 2026-11-08

The previous version is 0.1.3.

### Added

- `examples/web-stack.tfstate` and `examples/web-stack-next.tfstate`, two fictional states, with the expected output of `count --explain` and `diff` next to them. `tests/examples_golden.rs` compares the binary's output with those files.
- `README.md` and `README.nl.md` have an Example and an Options section (Voorbeeld and Opties in Dutch). `tests/readme.rs` checks that every option in `rum-meter --help` is in both.
- Manual page `man/rum-meter.1`, a fictional state in `examples/one-managed.tfstate`, and `tests/handbook_help.rs`. The test checks the long options in `rum-meter --help` against the manual page and runs the example command.

### Changed

- Every input error message now ends with a hint after a semicolon that says what you can do, for example `; pass each state file once`. The text before the semicolon, and every exit code, are unchanged.
- CI runs one workflow per ref at a time, has a job timeout, skips draft pull requests and caches the Cargo build.
- The copyright line in the licenses and READMEs reads `2025-2026 PEPAI / Pepijn de Jonge`.
- README and README.nl state the test count (237), the same number as the pre-release section of `QUALITY.md` and `QUALITY.nl.md`.

### Fixed

- The README said that passing the same state file to `count` twice was the `duplicate address` error. It is exit 2, `state file is given more than once`, before any file is opened, and the totals are not printed. The same address in two different files is still `duplicate address`.

## [0.1.3] - 2026-10-25

### Fixed

- A state file, price file or redirected stdin whose size is already over the 128 MiB cap is refused from its metadata before any byte is read. 0.1.2 read up to 128 MiB of a sparse file's zeros before refusing, which took about 9 seconds on a macOS Intel CI runner. The capped read loop stays for pipes and for files that grow after the size check.
- The sparse-file tests no longer time the refusal. A unit test counts the bytes the reader hands out and requires zero for a known size over the cap.

## [0.1.2] - 2026-10-25

### Fixed

- `count`'s 32 MiB shared label budget is a running total across every file in the invocation, not a per-file check. Several files each just under the budget, or one path repeated many times, no longer slip the module/mode/type/name copies through before the duplicate-address check can reject them. A repeated path is rejected before any file in the list is opened.
- `mode` is counted in the shared label budget before `classify` copies it onto each instance, closing a gap the earlier per-file budget left open.
- A state that repeats a shared resource label (module, mode, type or name) across too many instances is rejected instead of let through to classification.
- State, price and stdin reads stop as soon as the running total would exceed the 128 MiB cap, instead of trusting the file's logical size. A sparse file that reports a huge logical size while using no disk blocks no longer forces a read of that whole size before the JSON parser can fail.
- A price-file `currency` longer than 16 characters, or containing a character outside visible ASCII (U+0021 through U+007E), is refused at load, before it is copied onto every tier line.
- `diff` matches objects on module, mode, type, name, index key and deposed key, not on the printed address. A resource named `a[0]` and resource `a` at index 0 are now one removal and one addition instead of no change.
- A price file with two tiers of the same name is exit 2, `duplicate tier name`, and prints no cost.
- When a real module or type is named `other`, `--top` no longer prints two indistinguishable `other` rows. The remainder is `(other)`, or `(other N)` when several rows are folded together, so the names stay unique.
- A repeated key inside one price tier is `duplicate key in a tier`, not `tier is not an object`.
- Text output now escapes U+2028 (LINE SEPARATOR) and U+2029 (PARAGRAPH SEPARATOR) the same way it escapes other invisible or bidirectional format characters. An address, file path, module name, type or price label carrying one of these could otherwise print a raw character that several renderers treat as a line break, faking an extra line in a report.
- `docs/reference.md` and `docs/reference.nl.md` named a stale `--version` string (`rum-meter 0.1.0`). They now follow the crate version, and a test pins `--version` to that version.
- On edition 2024 the Windows `extern` block is `unsafe`, so the crate builds with that edition.
- `--prices -` is a missing file and exits 2. A price path has no stdin special case, and the test expects that exit code.
- The unix-only `File` import in the stdin test is compiled only on unix.
- `tests/cli.rs` matches `cargo fmt`.
- Test temporary directories stay off Windows device names such as `NUL`, and the suite guards against those names.

### Changed

- A state path `-` reads stdin when stdin is a pipe or a regular file. A terminal or a device on stdin is refused. The quick start includes a block that needs no file from this repository.
- A `cargo-fuzz` harness (`fuzz/`, excluded from the published crate) with one target per stage that reads untrusted input: state parsing, price-tier parsing, cost calculation, `diff`, and rendering. Each asserts no panic, that JSON output always parses, that amounts are never negative, NaN or infinite, and that fields the tool drops (`attributes`, `sensitive_attributes`, `private`, `outputs`) never reach a report. A cheap nightly build-only CI job keeps the targets buildable.
- `proptest` property tests that run under plain `cargo test`: the state and price parsers never panic on arbitrary bytes, the monthly cost matches an independent rational-arithmetic reference, and rendering the same verdicts twice is stable with JSON that always parses.
- Regression tests for newline-only files, a lone CR, an escaped NUL byte, Unicode spaces and lookalike letters, a duplicate JSON key outside a price tier, deeply nested JSON, and the numbers `1e309`, `-0` and an out-of-range integer.
- Release archives include third-party licenses, and CI checks the licenses of dependencies. `THIRD-PARTY-LICENSES.html` is generated once, on Linux, because there is no cargo-about binary for Intel macOS.
- `SECURITY.md` and a Dependabot config are in the repository. Dependabot opens cargo security updates only, so the MSRV stays 1.85, and it ignores `dtolnay/rust-toolchain` because that ref is the Rust version.
- The README shows downloads, MSRV, license and quality badges. The quality report for this line of work is in the tree, including the DONE final review of the diff-identity fix.
- CI and the release workflows use the Node 24 action versions. CI also runs the test suite in release mode.

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
