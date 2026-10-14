# Quality report

This page shows how each release of rum-meter was checked before it was published, what we found, and what we fixed. It describes checks we did. It is not a guarantee that the software is free of bugs or secure.

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
