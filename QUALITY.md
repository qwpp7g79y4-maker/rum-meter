# Quality report

This page shows how each release of rum-meter was checked before it was published, what we found, and what we fixed. It describes checks we did. It is not a guarantee that the software is free of bugs or secure.

## Pre-release 1.0.0 (branch prep/v1-kwaliteit)

| | |
|---|---|
| Version | 1.0.0 (not published from this branch) |
| Commit measured | `7e26916` |
| Tests | 268 passed, 0 failed, 0 ignored (`cargo test --no-fail-fast`, debug mode) |
| Lints | `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean |

## v0.1.2

| | |
|---|---|
| Version | 0.1.2 ([release](https://github.com/qwpp7g79y4-maker/rum-meter/releases/tag/v0.1.2), [crates.io](https://crates.io/crates/rum-meter/0.1.2)) |
| Commit checked | [`adc53d2`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/adc53d2) |
| Date | 2026-10-25 |
| Tests | 232 automated tests, run in debug and release mode |

### How we checked

- The same checks as for the previous version (below), on every platform.
- Automated tests in CI on Linux, Windows, macOS on Apple Silicon and Intel, and Rust 1.85, plus `cargo audit` (no known vulnerabilities) and `cargo deny check licenses` (ok).
- `cargo clippy --all-targets` and `cargo fmt --check` clean, on debug and release test runs.
- A `cargo-fuzz` harness (`fuzz/`) with one target per untrusted-input stage (state parsing, price-tier parsing, cost calculation, `diff`, rendering); CI only builds these targets on every push and pull request (`.github/workflows/ci.yml:76-88`), it does not run them, and there is no scheduled (nightly) trigger.

### What we found and fixed

| Problem | Fix |
|---|---|
| `diff` matched objects on the printed address, so a resource named `a[0]` and resource `a` at index 0 looked like no change | Matches on module, mode, type, name, index key and deposed key instead |
| A price file with two tiers of the same name was accepted | Exit 2, `duplicate tier name`, no cost printed |
| A repeated key inside one price tier was reported as `tier is not an object` | It is `duplicate key in a tier` |
| Text output did not escape U+2028/U+2029, so an address, path, module, type or price label carrying one of these could fake an extra line in a report | Escaped the same way as other invisible/bidirectional format characters |
| `docs/reference.md` and `docs/reference.nl.md` named a stale `--version` string (`rum-meter 0.1.0`) | Now follow the crate version; a test pins `--version` to it |
| The Windows `extern` block was not `unsafe`, so the crate did not build on edition 2024 | Marked `unsafe` |
| A price-file `currency` longer than 16 characters, or outside visible ASCII, was copied onto every tier line before anything rejected it | Refused at load, before any copy exists |
| A state could repeat a shared resource label (module, mode, type or name) across too many instances before classification caught it | Rejected up front, before classification |
| A sparse file's logical size, not its real content, drove how much of a state, price or stdin read would be allocated and read before the JSON parser could fail | Reads stop as soon as the running total would exceed the 128 MiB cap |
| `count`'s 32 MiB shared label budget was checked once per file, so several files each just under the budget, or one repeated path, slipped through | Budget is a running total across the whole invocation; a repeated path is rejected before any file opens |
| `mode` was copied onto each instance by `classify` before it was counted against the shared label budget | Counted in the budget first |

All fixes are listed in the [changelog](https://github.com/qwpp7g79y4-maker/rum-meter/blob/main/CHANGELOG.md).

## v0.1.1

| | |
|---|---|
| Version | 0.1.1 ([release](https://github.com/qwpp7g79y4-maker/rum-meter/releases/tag/v0.1.1), [crates.io](https://crates.io/crates/rum-meter/0.1.1)) |
| Commit checked | [`d6f3fe2`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/d6f3fe2) |
| Date | 2026-10-15 |
| Tests | 176 automated tests, run in debug and release mode |

### How we checked

- The same checks as for the previous version (below), on every platform.
- A second external automated code review. Every finding we could reproduce is fixed in this version, each with a regression test that uses the exact input.
- Automated tests in CI: [run 38654547261](https://github.com/qwpp7g79y4-maker/rum-meter/actions/runs/38654547261) on Linux, Windows, macOS on Apple Silicon and Intel, and Rust 1.85, plus `cargo audit` (no known vulnerabilities).
- A fresh independent final review on this commit against a fixed edge-case checklist (every item tried with a concrete bad input). It ended DONE; no blocking findings.
- Unreleased: a later fresh final review of commit [`195b64c`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/195b64c) found that `diff` could show a real removal and addition as no change when two objects print the same address (a resource named `a[0]` and resource `a` at index 0). That is fixed in [`37e050a`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/37e050a), with regression tests. A fresh independent final review of commit [`37e050a`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/37e050a), against the same fixed checklist with every item tried with a concrete bad input, ended DONE, with no blocking findings.

### What we found and fixed

| Problem | Fix |
|---|---|
| A price could be rounded through binary floating point (off by one cent) | Prices are read as exact decimals |
| A state file whose root is a JSON array was accepted | It is refused with exit 2 |
| The same address given twice across input files was counted twice | It is refused |
| A path swapped for a FIFO could make the tool wait; a full disk on stdout or stderr caused a panic | The opened file is checked; write errors are clean errors and never exit 0 |
| `--top` produced two indistinguishable `other` rows when a real module or type was named `other` | The remainder is `(other)` (then `(other 2)`, …) so every name stays unique |
| A repeated key in a price tier was reported as `tier is not an object` | It is `duplicate key in a tier` (exit 2) |

All fixes are listed in the [changelog](https://github.com/qwpp7g79y4-maker/rum-meter/blob/main/CHANGELOG.md).

## v0.1.0

| | |
|---|---|
| Version | 0.1.0 ([release](https://github.com/qwpp7g79y4-maker/rum-meter/releases/tag/v0.1.0), [crates.io](https://crates.io/crates/rum-meter/0.1.0)) |
| Commit checked | [`32f8402`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/32f8402) |
| Date | 2026-10-11 |
| Tests | 158 automated tests |

### What we checked

**Code review.** The code was read against nine fixed questions:
1. Does it give the right answer?
2. What happens with bad or missing input?
3. What happens when it fails halfway, for example when output cannot be written?
4. Which assumptions does it make, and are they written down?
5. Is it safe when things happen at the same time?
6. Can untrusted input leak into the output or do harm, for example through hidden characters?
7. Does it stay fast and small with large input?
8. Is every claim backed by a test?
9. Is the code easy to read and change?

**Input from every platform.** UTF-8 with and without a byte order mark, UTF-16 and UTF-32, Windows line endings (CRLF), and file paths that are not valid UTF-8.

**Tests on every platform.** Every change runs the full test suite on Linux, Windows, macOS on Apple Silicon and macOS on Intel, plus the oldest supported Rust version (1.85).

**Release.** The release is built only after the tests pass again on the tagged commit. Each download is unpacked and started once before publishing, and gets a SHA-256 checksum.

**Dependencies.** `cargo audit` on 2026-10-12 against the RustSec advisory database (1296 advisories): no known vulnerabilities or warnings in the 35 crate dependencies. It now runs in CI on every change.

### How we checked

- Independent code review, by reviewers who did not write the code.
- An external automated code review.
- Automated tests in CI. Runs: [release v0.1.0](https://github.com/qwpp7g79y4-maker/rum-meter/actions/runs/38174144815), [latest main](https://github.com/qwpp7g79y4-maker/rum-meter/actions/runs/38203196595).

We do not name the review services here. The checks and their results are what matters, and every result below can be verified in the code and the changelog.

### What we found and fixed before release

| Problem | Fix |
|---|---|
| Costs could be rounded wrongly because of floating point | Decimal arithmetic, rounded half away from zero to whole cents |
| An overflow was possible at 0 hours | 0 hours always gives 0.00 |
| Control characters in addresses, labels and paths could garble the report | They are escaped in text output |
| Invisible and bidirectional characters could make a line read differently than it is | They are escaped in text output |
| A failed write to stdout (for example piping into `head`) was not reported | Exit code 2 with a clear message |
| UTF-16 and UTF-32 files were not handled clearly | They are refused with a clear message |
| A UTF-8 byte order mark broke parsing | It is skipped |

All fixes are part of the first public version, see the [changelog](https://github.com/qwpp7g79y4-maker/rum-meter/blob/main/CHANGELOG.md#010---2026-10-11).

### Not covered

- Only Terraform state format version 4 is supported.
- The cost estimate uses prices you supply. It is not an official bill.

## Report a problem

Open an [issue](https://github.com/qwpp7g79y4-maker/rum-meter/issues), or for security problems follow [SECURITY.md](SECURITY.md). Contact: pep_ai@icloud.com.
