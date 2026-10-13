# rum-meter

English · [Nederlands](README.nl.md)

Count billable HCP Terraform resources in a local state file, before the invoice.

[![crates.io](https://img.shields.io/crates/v/rum-meter.svg)](https://crates.io/crates/rum-meter) [![CI](https://github.com/qwpp7g79y4-maker/rum-meter/actions/workflows/ci.yml/badge.svg)](https://github.com/qwpp7g79y4-maker/rum-meter/actions/workflows/ci.yml)

[Installation](#installation) • [Quick start](#quick-start) • [How it counts](#how-it-counts) • [Alternatives](#compared-with-alternatives) • [Limitations](#limitations) • [Reference](docs/reference.md)

rum-meter is a command-line tool written in Rust. It reads `terraform.tfstate` files of format version 4 and counts billable HCP Terraform Resources Under Management (RUM) from HashiCorp's definition. It explains each instance, breaks the total down by module and by type, estimates a monthly cost from a price file you supply, and diffs two states. It needs no account and makes no network connection. The result is an estimate.

## Why

HCP Terraform bills per RUM. A team sees that count on the billing page afterwards, without a per-resource explanation. rum-meter shows, beforehand and on your machine, how much RUM a state has, which module and type it sits in, and what a change adds or removes. Willingness to pay was not investigated. The HashiCorp sentences this follows were checked on 7 October 2026 and are quoted under [Compared with alternatives](#compared-with-alternatives).

## Installation

With Rust 1.85 or newer, install from [crates.io](https://crates.io/crates/rum-meter):

```sh
cargo install --locked rum-meter
rum-meter --help
```

Ready-made binaries for Linux (x86_64, glibc and musl), Windows (x86_64) and macOS (Intel and Apple Silicon), each with a SHA-256 checksum, are on the [Releases page](https://github.com/qwpp7g79y4-maker/rum-meter/releases). Unpack the archive and put `rum-meter` on your `PATH`.

For the latest `main`: `cargo install --locked --git https://github.com/qwpp7g79y4-maker/rum-meter`.

From a local clone: `cargo install --locked --path .`, or `cargo build --release`; the binary is `target/release/rum-meter`.

## System requirements

| Platform | Release archive | Requirement |
|---|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | glibc 2.34 or newer (for example Ubuntu 22.04, Debian 12, RHEL 9) |
| Linux x86_64 | `x86_64-unknown-linux-musl` | none; statically linked, also for Alpine and older distributions |
| macOS Apple Silicon | `aarch64-apple-darwin` | macOS 11 or newer |
| macOS Intel | `x86_64-apple-darwin` | macOS 10.12 or newer |
| Windows x86_64 | `x86_64-pc-windows-msvc` | Windows 10 or newer |

The binaries are built and smoke-tested on GitHub runners (Ubuntu, macOS on Apple Silicon and Intel, Windows). Building from source needs Rust 1.85 or newer on any platform Rust supports.

## Quick start

These are the real outputs of the binary on the synthetic fixtures in `tests/fixtures`. A test runs every example and compares the output.

Count one state:

```console
$ rum-meter count tests/fixtures/v4_one_managed.json
file: tests/fixtures/v4_one_managed.json
rum: 1
not counted: R2 0, R3 0, R4 0
total rum: 1
not counted: R2 0, R3 0, R4 0
modules:
  (root): 1
types:
  random_id: 1
estimate: HCP Terraform bills the hourly peak; this is a single snapshot
```

`--explain` adds one line per instance under the file: the address, `counts` or `does not count`, and the rule code. A tainted object appends ` tainted`.

```console
$ rum-meter count --explain tests/fixtures/v4_handwritten.json
file: tests/fixtures/v4_handwritten.json
rum: 2
not counted: R2 1, R3 2, R4 1
  null_resource.counted[0] does not count R3
  null_resource.counted[3] does not count R3
  module.child.random_id.labeled["web"] counts R1
  module.child.random_id.labeled["db"] counts R1
  module.child.module.nested.data.local_file.cfg does not count R2
  module.child.module.nested.terraform_data.unused does not count R4
total rum: 2
not counted: R2 1, R3 2, R4 1
modules:
  module.child: 2
types:
  random_id: 2
estimate: HCP Terraform bills the hourly peak; this is a single snapshot
```

An absent module is `(root)`. Rows are ordered by count descending, then by name ascending. `--top N` keeps the first N rows of each list. The rest is one row `other`.

```console
$ rum-meter count --top 3 tests/fixtures/v4_top.json
file: tests/fixtures/v4_top.json
rum: 16
not counted: R2 1, R3 3, R4 1
total rum: 16
not counted: R2 1, R3 3, R4 1
modules:
  module.compute: 5
  module.network: 5
  module.app: 3
  other: 3
types:
  local_file: 4
  random_id: 4
  random_integer: 3
  other: 5
estimate: HCP Terraform bills the hourly peak; this is a single snapshot
```

`--json` replaces the text with one object. `schema_version` is 1.

```console
$ rum-meter count --json tests/fixtures/v4_one_managed.json
{
  "schema_version": 1,
  "tool": "rum-meter",
  "estimate": "HCP Terraform bills the hourly peak; this is a single snapshot",
  "inputs": [
    "tests/fixtures/v4_one_managed.json"
  ],
  "total_rum": 1,
  "by_module": [
    {
      "name": "(root)",
      "count": 1
    }
  ],
  "by_type": [
    {
      "name": "random_id",
      "count": 1
    }
  ]
}
```

Costs appear only with `--prices`. The price file is input: a currency, and for each tier a name and a price per RUM-hour. A month is RUM times that price times the number of hours. The default is 730 hours. `--hours-per-month` sets another number. On the page 'Estimate HCP Terraform cost', HashiCorp's example uses "Per Hour Price x 24 x 30" (720 hours). rum-meter defaults to 730, and `--hours-per-month 720` follows that example. `tests/fixtures/example-prices.json` is a made-up example, not a HashiCorp rate. The `note` in that file is Dutch. The tool ignores `note` and does not print it.

```json
{
  "note": "Example. Not a HashiCorp rate.",
  "currency": "EUR",
  "tiers": [
    {
      "name": "example-a",
      "price_per_rum_hour": 1
    },
    {
      "name": "example-b",
      "price_per_rum_hour": 0.5
    }
  ]
}
```

One RUM on that example, for 730 hours, is 730.00 and 365.00. Those amounts follow from the example file, not from a HashiCorp price list. A comma is not a decimal separator (`0.5`, not `0,5`).

```console
$ rum-meter count --prices tests/fixtures/example-prices.json tests/fixtures/v4_one_managed.json
file: tests/fixtures/v4_one_managed.json
rum: 1
not counted: R2 0, R3 0, R4 0
total rum: 1
not counted: R2 0, R3 0, R4 0
modules:
  (root): 1
types:
  random_id: 1
costs:
  example-a: EUR 730.00
  example-b: EUR 365.00
estimate: HCP Terraform bills the hourly peak; this is a single snapshot
```

The same file with 720 hours:

```console
$ rum-meter count --prices tests/fixtures/example-prices.json --hours-per-month 720 tests/fixtures/v4_one_managed.json
file: tests/fixtures/v4_one_managed.json
rum: 1
not counted: R2 0, R3 0, R4 0
total rum: 1
not counted: R2 0, R3 0, R4 0
modules:
  (root): 1
types:
  random_id: 1
costs:
  example-a: EUR 720.00
  example-b: EUR 360.00
estimate: HCP Terraform bills the hourly peak; this is a single snapshot
```

`diff` compares two states: added and removed counting addresses, instances that change rule, and the RUM difference. With `--prices` it also prints the monthly cost difference per tier.

```console
$ rum-meter diff tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json
old: tests/fixtures/v4_diff_old.json
new: tests/fixtures/v4_diff_new.json
rum: 6 -> 4
rum delta: -2
added:
  random_id.servers[1]
removed:
  local_file.note
  module.child.random_id.labeled["gone"]
  random_id.marker
changed rule:
estimate: HCP Terraform bills the hourly peak; this is a single snapshot
```

Exit code 0 is success. Exit code 1 is a threshold: `--max-rum` on `count`, `--max-increase` on `diff`. The report is still printed. Exit code 2 is an input error (unreadable file, not a regular file, invalid JSON, a state that is not a JSON object, state version not 4, a duplicate address, UTF-16 or UTF-32, an invalid price file, or a cost that overflows). Exit code 3 is a usage error. A flag that would not change the report is also exit 3, with a fixed message on stderr and nothing on stdout, before the states are read. If several apply, the first of these wins:

- `--max-increase only works with diff`
- `--max-rum only works with count`
- `--hours-per-month only works together with --prices`
- `--top with diff only works together with --json`
- `--explain with diff only works together with --json`

A report that cannot be written to stdout is exit 2, with `rum-meter: cannot write report to stdout`, and a closed stdout is the same error. `--version` and `--help` that cannot be written are exit 2, not 0, with `rum-meter: cannot write message to stdout`. A failed write does not panic, even when stderr cannot be written.

All options, exit codes and error messages are listed in the [reference](docs/reference.md) and in `rum-meter --help`.

## How it counts

Every number can be traced back to these rules. The address format, the price file and the JSON object are in the [reference](docs/reference.md#how-it-counts).

- **R1 counts:** a resource with `mode` `managed`, including a resource that exists only locally. With `count` and `for_each`, each instance counts. A tainted object counts under R1 and keeps the same address. `--explain` and JSON mark it as tainted.
- **R2 does not count:** `mode` `data` (a data source is not a managed resource), with the reason `data source`. Assumption: an unknown mode (not `managed` and not `data`) is not a managed resource, does not count, keeps R2, and gets the reason `unknown mode`.
- **R3 does not count:** type `null_resource` or `terraform_data` ("Null Resources" are excluded). Assumption: `terraform_data` falls under "Null Resources".
- **R4:** a resource block with no instances counts 0.
- **Deposed:** a deposed object is its own infrastructure object. The address ends with ` (deposed KEY)`, for example `local_file.a (deposed 00000001)`. A counting deposed object stays on R1. A deposed `null_resource` or `terraform_data` stays on R3. There is no separate rule code for it.
- **Several files** are summed. Only counting instances (R1) enter the module and type lists. The same module path or type from several states is added up. The same address in more than one of those files is not summed: that is exit 2, `duplicate address`, with the address. Passing the same file twice is that error.
- **Share of a list:** rows are ordered by count descending, then by name ascending. `--top N` keeps the first N rows. The rest is one final row `other`. Without `--top`, or when nothing remains, there is no `other` row. An absent or empty module is `(root)`.
- **Cost:** per month = RUM x price per RUM-hour x hours. The default is 730 hours. The price is the exact decimal from the file. 1 RUM at 0.004999999999999999999 for 1 hour is 0.00. 1 RUM at 1.005 for 1 hour is 1.01. Amounts are rounded half away from zero to 2 decimals, and the decimal separator in the output is a point. `diff` subtracts the rounded cent amounts. A product that overflows is exit 2, never a clamped amount.
- **Estimate:** the text always ends with `estimate: HCP Terraform bills the hourly peak; this is a single snapshot`. JSON uses the same sentence without the `estimate:` prefix.

Worked example (the quick start above): 1 RUM at 1 per RUM-hour for 730 hours is 730.00. At 0.5 per RUM-hour that is 365.00. With `--hours-per-month 720` the same prices are 720.00 and 360.00.

## Compared with alternatives

An honest comparison with HCP Terraform's own billing, which is the definition rum-meter follows. The pages were checked on 7 October 2026. Anything those pages do not state is marked here as not verified.

**HCP Terraform** (https://developer.hashicorp.com/hcp/docs/hcp/admin/billing/pricing-definitions) defines what counts as a resource under management. The page's sentence "Each infrastructure object, including but not limited to “Local Only” Resources, counts as a single Resource." is the basis for counting every object, including one that exists only locally. The same page is the source for "Hourly Peak Managed Resources": the bill uses the maximum number of simultaneous managed resources in a billing hour, summed across the HCP Terraform organizations under one HCP organization. rum-meter cannot see that peak. It counts one snapshot and always says so.

https://developer.hashicorp.com/terraform/cloud-docs/overview says "A managed resource is a resource in an HCP Terraform-managed state file where `mode = "managed"`. HCP Terraform counts a resource as part of this count starting from the first `terraform plan` or `terraform apply` operation on the resource." rum-meter does not see that history. It includes every instance in the file you pass. Whether a resource in that file has had a plan or apply is not verified.

https://developer.hashicorp.com/terraform/cloud-docs/workspaces/state says "HCP Terraform includes resources in modules and each resource instance created with the `count` or `for_each` meta-arguments." rum-meter does the same for a state file on disk.

On the page 'Estimate HCP Terraform cost', the worked example is "Per Hour Price x 24 x 30" (720 hours). rum-meter's default month is 730 hours. Pass `--hours-per-month 720` for that example. The prices on a HashiCorp price list are not in this tool. Whether the current list still uses that 720-hour example is not verified beyond the sentence quoted here.

Terraform writes `index_key`, `status` (only `tainted`) and `deposed` per object in state v4 (`internal/states/statefile/version4.go`, commit b6daf8bc, lines 705-707). Assumption, because it is not a separate sentence on the pricing page: a deposed object counts as its own infrastructure object. That reading is not verified as a HashiCorp billing rule. It follows the sentence about every infrastructure object.

The billing page shows the count afterwards. rum-meter reads a file you already have, explains each instance with a rule code, and does not send the state anywhere. It does not replace the invoice.

## Limitations

- The official bill is the hourly peak: the maximum number of simultaneous managed resources per billing hour, summed across the HCP Terraform organizations under one HCP organization. That peak cannot be measured offline. rum-meter counts one snapshot per file and calls the result an estimate.
- The definition counts a resource when it is in an HCP Terraform-managed state and has had at least one plan or apply. rum-meter does not see that history and includes every instance in the given file.
- Only state format v4 (Terraform 0.12 and newer) is read, and the root value has to be a JSON object. A JSON array is exit 2, `state is not an object`. Remote state and plan files stay outside the tool. Count several workspaces by passing several files. The sum is not per hour and not per organization. An address that also occurs in another of those files is exit 2, `duplicate address`, and is not counted twice.
- One workspace is one state. Flex, contracts and other discounts are included only when they are in the price file. rum-meter does not check those numbers. Old free-tier rules and Terraform Enterprise licenses have no model of their own.
- "Each partial hour is billed as a full hour". Rounding per hour is not modeled.
- "HCP Terraform combines workspace and Stacks resources under management when calculating costs". Stacks state is not supported.
- The free plan with at most 500 managed resources is a limit, not a price.
- A state should contain each address once. If an address appears twice, at the root, in a module, with count or for_each, or as a deposed object, rum-meter does not count that state: exit 2, with the address and the reason, without attribute values. The same rule applies when `count` is given several files: the file that repeats an address is exit 2, and the totals are not printed. On `diff`, added and removed therefore still add up to the RUM delta.
- rum-meter reads UTF-8, for state files and price files. A UTF-8 BOM at the start of the file is skipped. A UTF-16 or UTF-32 file, with or without a BOM, such as Windows PowerShell 5.1 writes on a redirect, is not converted. That is exit 2, with the message that the file is UTF-16 (or UTF-32) and must be saved as UTF-8, for example with `iconv -f UTF-16 -t UTF-8` or in PowerShell with `Out-File -Encoding utf8`. CRLF line ends are fine.
- A file name that is not valid UTF-8 (possible on Linux) is read, but shown in the report with U+FFFD for the invalid bytes.
- rum-meter reads only regular files, so not a pipe or process substitution. A directory, a FIFO or a device is refused with exit 2 before any byte is read. A symlink to a FIFO is refused the same way. The open does not wait, and the type is taken from the opened file, so a path that becomes a FIFO is not read and does not hang. A symlink to a regular file is read.

Where the fixtures come from: [tests/fixtures/README.md](tests/fixtures/README.md).

## Privacy

rum-meter is fully offline: no network, no API key, no telemetry. The binary reads only the paths you pass. Output contains addresses, module paths, types, rule codes and counts. Attribute values, sensitive values, private data and outputs are left out. `tests/safety.rs` guards that with synthetic fixtures. No error message contains bytes from the file, only the path, the line and column of a parse error, and the kind of error.

## Support

rum-meter is free to use. If it saves you money, you can support the work via [Ko-fi](https://ko-fi.com/pepai).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Copyright 2026 PEPAI.

## Contact

PEPAI, pep_ai@icloud.com

Report bugs and ideas via [Issues](https://github.com/qwpp7g79y4-maker/rum-meter/issues); report security problems privately as described in [SECURITY.md](SECURITY.md).
