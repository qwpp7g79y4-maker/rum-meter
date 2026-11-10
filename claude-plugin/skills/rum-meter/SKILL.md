---
name: rum-meter
description: Estimate billable HCP Terraform RUM from local Terraform state files, offline. Use when the user mentions Terraform Cloud or HCP Terraform cost, resources under management, a state file, or rum-meter.
---

# rum-meter

Use the local `rum-meter` CLI (version 0.1.3). It estimates billable HCP Terraform RUM from local state files, offline. It does not call the HCP Terraform API.

## Install

If `rum-meter` is not on `PATH`, install it with Rust 1.85 or newer:

```sh
cargo install --locked rum-meter
rum-meter --help
```

Do not invent flags. If you are unsure, run `rum-meter --help`.

## Commands (from `--help`)

```
rum-meter [OPTIONS] <COMMAND>
```

- `count` — Count billable RUM in one or more state files. Usage: `rum-meter count [OPTIONS] <STATE>...`
- `diff` — Compare billable RUM in two state files. Usage: `rum-meter diff [OPTIONS] <OLD> <NEW>`

A state argument `-` reads stdin. Any other state path is a file path.

Options:

- `--json` — Print JSON instead of text
- `--explain` — Print one line per instance
- `--top <N>` — List at most N modules and N resource types
- `--prices <FILE>` — Read tier prices from this JSON file
- `--hours-per-month <N>` — Hours in a month for the cost estimate (only together with `--prices`; default is 730 when costs are shown)
- `--max-rum <N>` — RUM limit for count (exit 1 when over; report is still printed)
- `--max-increase <N>` — RUM increase limit for diff (exit 1 when over; report is still printed)
- `-h`, `--help` / `-V`, `--version`

Rejected combinations exit 3, with these messages, in this order:

- `--max-increase only works with diff`
- `--max-rum only works with count`
- `--hours-per-month only works together with --prices`
- `--top with diff only works together with --json`
- `--explain with diff only works together with --json`

Every text report ends with: `estimate: HCP Terraform bills the hourly peak; this is a single snapshot`. Do not invent HashiCorp list prices. Costs appear only with `--prices`.

## Examples from the project README

No repository files needed:

```sh
cat <<'EOF' | rum-meter count -
{
  "version": 4,
  "resources": [
    {
      "mode": "managed",
      "type": "random_id",
      "name": "one",
      "instances": [{}]
    }
  ]
}
EOF
```

Fixture examples (need a clone):

```sh
rum-meter count tests/fixtures/v4_one_managed.json
rum-meter count --explain tests/fixtures/v4_handwritten.json
rum-meter count --top 3 tests/fixtures/v4_top.json
rum-meter count --json tests/fixtures/v4_one_managed.json
rum-meter count --prices tests/fixtures/example-prices.json tests/fixtures/v4_one_managed.json
rum-meter count --prices tests/fixtures/example-prices.json --hours-per-month 720 tests/fixtures/v4_one_managed.json
rum-meter diff tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json
```

The example price file in the repository is made-up, not a HashiCorp rate.

## Exit codes

- 0 — success
- 1 — `--max-rum` or `--max-increase` threshold
- 2 — input error (unreadable file, invalid JSON, state version not 4, duplicate address, invalid price file, or a report that cannot be written)
- 3 — usage error
