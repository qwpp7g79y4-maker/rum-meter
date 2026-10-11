# Contributing

Thanks for your interest in rum-meter.

## Building and testing

You need Rust 1.85 or newer and Python 3: the JSON tests check the output with Python's `json` module (standard library only). The tests run `python3`, or `python` if `python3` is not found (as on many Windows installs).

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

These checks must pass before a change is merged, on stable and on Rust 1.85 (`cargo +1.85 clippy ...`, `cargo +1.85 test ...`). Test fixtures must be synthetic: never add a real state or a real secret.

## Issues

Use the [issue forms](https://github.com/qwpp7g79y4-maker/rum-meter/issues/new/choose) for bugs and feature requests. Never paste a real state file: it can hold secrets in plain text. Report security problems privately as described in [SECURITY.md](SECURITY.md).

## Commit messages

Commits follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) in English, for example `fix: refuse a FIFO as price file` or `docs: clarify the sample rounding`.

## Contact

PEPAI, pep_ai@icloud.com
