# RUM Meter

This plugin helps Claude use the `rum-meter` command-line tool. The tool estimates billable HCP Terraform RUM (resources under management) from local Terraform state files. It works offline and does not call the HCP Terraform API.

## Use it

Install the tool with Rust 1.85 or newer: `cargo install --locked rum-meter`. Give Claude one or more state files, or pipe a state on standard input with `-`, and ask how much RUM it would bill, or how two states differ. Monthly costs appear only when you pass your own price file with `--prices`.

## Data

The tool is fully offline: no network, no API key, and no telemetry. It reads only the paths you pass, or standard input when a state path is `-`. Output contains addresses, module paths, types, rule codes, and counts. Attribute values, sensitive values, private data, and outputs are left out. This plugin stores nothing and sends nothing to any service. The printed estimate is a single snapshot; HCP Terraform bills the hourly peak.
