# rum-meter reference

English · [Nederlands](reference.nl.md)

This page has the complete details. For an overview, see the [README](../README.md).

## Options

Run `rum-meter --help`, `rum-meter count --help` or `rum-meter diff --help` for the built-in help. `count` takes one or more state files (`STATE`). `diff` takes the earlier file (`OLD`) and then the later file (`NEW`).

A flag that would not change the report is exit 3, with one fixed line on stderr and nothing on stdout, before any file is read. If several apply, the first match wins: `--max-increase` on count, `--max-rum` on diff, `--hours-per-month` without `--prices`, then on diff without `--json` first `--top` and then `--explain`. `--json` does not change a refusal. With `--json`, `--top` and `--explain` on diff do apply, and the command exits 0. See [Exit codes and usage errors](#exit-codes-and-usage-errors).

| Option | Applies to | What it does |
| --- | --- | --- |
| `--json` | count and diff | Print one JSON object instead of text. `schema_version` is 1. |
| `--explain` | count; on diff only together with `--json` | One line per instance: the address, `counts` or `does not count`, and the rule code. A tainted object appends ` tainted`. |
| `--top` | count; on diff only together with `--json` | Keep at most N module rows and N type rows. The rest is one row `other`, or `(other)` when that name is already taken. |
| `--prices` | count and diff | Read tier prices from this JSON file. Without it there is no cost line. |
| `--hours-per-month` | count and diff, only together with `--prices` | Hours in the month used for the estimate. Default 730. |
| `--max-rum` | count | Exit 1 when the summed RUM is above N. N itself is within the limit. The report is still printed. |
| `--max-increase` | diff | Exit 1 when new RUM minus old RUM is above N. An equal delta is within the limit. A decrease is within every limit. The report is still printed. |
| `--help` | everywhere | Show help and exit 0. `-h` is the short form. |
| `--version` | top level | Show `rum-meter 0.1.0` and exit 0. `-V` is the short form. |

## Price file

Prices come only from a JSON file you pass with `--prices`. The program has no built-in prices. This is the synthetic fixture `tests/fixtures/example-prices.json`:

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

`currency` is a non-empty string. `tiers` is a non-empty array of objects, each with a unique `name`. Each object has a non-empty `name` and a `price_per_rum_hour` that is a finite number, zero or greater. A repeated key in a tier is `duplicate key in a tier` (exit 2); a repeated tier name is `duplicate tier name` (exit 2). `0` is a valid price. Any other field, including `note`, is ignored and is not printed. The note in the example is Dutch. The file is an example, not a HashiCorp rate.

A comma is not a decimal separator (`0.5`, not `0,5`). An unquoted `0,5` is invalid JSON. A quoted `"0,5"` is `price is not a number`.

Other price-file failures, each one line on stderr and nothing on stdout:

- not an object, for example a JSON array: `invalid price file`
- missing or empty currency: `currency is empty`
- missing `tiers`, a `tiers` value that is not an array, or an empty array: `price file has no tiers`
- a tier that is not an object: `tier is not an object`
- a repeated key in a tier: `duplicate key in a tier`
- an empty tier name: `tier name is empty`
- two tiers with the same name: `duplicate tier name`
- a price that is missing, not a number, or not finite: `price is not a number`
- a negative price: `price is negative`
- the path is not a regular file: `not a regular file`
- the path cannot be opened: `unreadable (not found)`, `unreadable (permission denied)`, `unreadable (is a directory)` or `unreadable (io error)`
- the bytes are not JSON: `invalid json at line L column C`

A month is RUM times the price per RUM-hour times the hours. The default is 730 hours. The price is the exact decimal from the file, so 1 RUM at `0.004999999999999999999` for 1 hour is `0.00`. The calculation uses decimal arithmetic, so 1 RUM at 1.005 for 1 hour is `1.01`. The product is multiplied by 100 and rounded half away from zero to a whole number of cents, then shown as a number of euros, a point, and two digits. `diff` subtracts those cent integers. A positive cost delta is prefixed with `+`, a negative delta with `-`, and zero has no sign. 0 hours gives `0.00`. 1 RUM at 0.125 for 1 hour is `0.13`. 1 RUM at 0.0625 for 1 hour is `0.06`.

On the example file, 1 RUM for 730 hours is `730.00` and `365.00`. For 720 hours it is `720.00` and `360.00`. Those amounts follow from the example file, not from a HashiCorp price list.

On the page 'Estimate HCP Terraform cost', the worked example is "Per Hour Price x 24 x 30" (720 hours). Pass `--hours-per-month 720` for that example. Whether the current list still uses that 720-hour example is not verified beyond the sentence quoted here.

A product that is not finite, is negative, or does not fit in the cent amount is exit 2: `monthly cost overflows`. The amount is never clamped. An input error is printed before any report, so it wins over a threshold.

## State file

Only state format version 4 is accepted, and `version` has to be a number. The root value has to be a JSON object. A JSON array is `state is not an object`. A string is `invalid json at line L column C`. Any other number is `state version is not 4`.

The reader stores `version`, and for each resource `mode`, `type`, `name`, `module` and `instances`. For each instance it stores `index_key` (a number or a string), the `deposed` key, and whether `status` is exactly the string `tainted`. Any other status is dropped. Unknown JSON fields are ignored. `attributes`, `sensitive_attributes`, `private` and `outputs` are not deserialized and are never stored.

The file must be UTF-8; the same holds for a price file. A leading UTF-8 BOM (the 3 bytes `EF BB BF`) is skipped. A UTF-16 BOM (`FF FE` or `FE FF`), such as Windows PowerShell 5.1 writes on a redirect, is not converted. That is exit 2: `file is UTF-16; save it as UTF-8`. A UTF-32 BOM (`FF FE 00 00` or `00 00 FE FF`) gives `file is UTF-32; save it as UTF-8`. Without a BOM the encoding is recognised by the NUL bytes among the first 4 bytes, as in RFC 4627: a JSON text starts with an ASCII character. Nothing of such a file is decoded. CRLF line ends count the same as LF. A path that is not valid UTF-8 is read and shown with U+FFFD for the invalid bytes, in text and in JSON.

Only a regular file is read, for a state and for a price file. The file is opened without waiting, and the type is taken from that opened file, before any byte of it is read. A directory, a FIFO, a device, or a symlink to one of those is `not a regular file`. A path that becomes a FIFO is not read and does not hang. A symlink to a regular file is read. That is why a pipe or process substitution does not work. The state argument `-` reads stdin instead, to the end, when stdin is a pipe or a regular file. A terminal or a device on stdin is `not a regular file`. A price path is always a file. A missing path is `unreadable (not found)`. The other unreadable kinds are `permission denied`, `is a directory` and `io error`.

Each address may occur once, in one file and across the files passed to `count`. A repeat, at the root, in a module, with count or for_each, as a deposed object, or in a later file, is exit 2: `duplicate address`, plus the address. Control characters, and invisible or bidirectional format characters such as U+200B and U+202E, in that address are written as `\uXXXX`. Attribute values are not included. The first repeat in verdict order is the one reported. On `diff`, added and removed therefore still add up to the RUM delta. Text output escapes control characters the same way. JSON keeps the characters and lets the JSON encoder escape them.

## How it counts

The pages below were checked on 7 October 2026. Anything they do not state is marked as not verified.

https://developer.hashicorp.com/hcp/docs/hcp/admin/billing/pricing-definitions says "Each infrastructure object, including but not limited to “Local Only” Resources, counts as a single Resource." That sentence is the basis for counting every object, including one that exists only locally. The same page is the source for "Hourly Peak Managed Resources": the bill uses the maximum number of simultaneous managed resources in a billing hour, summed across the HCP Terraform organizations under one HCP organization. rum-meter cannot see that peak. It counts one snapshot and always says so.

https://developer.hashicorp.com/terraform/cloud-docs/overview says "A managed resource is a resource in an HCP Terraform-managed state file where `mode = "managed"`. HCP Terraform counts a resource as part of this count starting from the first `terraform plan` or `terraform apply` operation on the resource." rum-meter does not see that history. It includes every instance in the file you pass. Whether a resource in that file has had a plan or apply is not verified.

https://developer.hashicorp.com/terraform/cloud-docs/workspaces/state says "HCP Terraform includes resources in modules and each resource instance created with the `count` or `for_each` meta-arguments." rum-meter does the same for a state file on disk.

Precedence for one resource block:

- No instances: R4. It counts 0. The classification reason is `resource has no instances`.
- Type `null_resource` or `terraform_data`: R3, including when `mode` is `data`. The reason is `null resource`. Assumption: `terraform_data` falls under "Null Resources".
- `mode` `managed`: R1. It counts. The reason is `managed resource`. A resource that exists only locally still counts.
- `mode` `data`: R2. It does not count. The reason is `data source`.
- Any other mode: R2. It does not count. Assumption: an unknown mode (not `managed` and not `data`) is not a managed resource, does not count, keeps R2, and gets the reason `unknown mode`.

The reason is a classification label. The text report does not print it. An explain line is the address, then `counts` or `does not count`, then the rule code (`R1`, `R2`, `R3` or `R4`). A tainted object appends ` tainted`. JSON `rule` is that same code. There is no reason field. `tainted` is present only when it is true. `deposed` is present only when the object has a deposed key.

With `count` and `for_each`, each instance is its own row. The address is `module.a.module.b.type.name`, plus `[n]` for a count index or `["key"]` for a for_each key. A `"` or `\` inside the key is escaped with a backslash. A data source inserts `data.` before the type, so `local_file.a` and `data.local_file.a` are different addresses. An empty module is left off the address. A non-empty deposed key is appended as ` (deposed KEY)`, for example `local_file.a (deposed 00000001)`. An empty deposed string is ignored. A counting deposed object stays on R1. A deposed null type stays on R3. There is no separate rule code for it.

Terraform writes `index_key`, `status` (only `tainted`) and `deposed` per object in state v4 (`internal/states/statefile/version4.go`, commit b6daf8bc, lines 705-707). Assumption, because it is not a separate sentence on the pricing page: a deposed object counts as its own infrastructure object. That reading is not verified as a HashiCorp billing rule. It follows the sentence about every infrastructure object.

The line `not counted: R2 n, R3 n, R4 n` is printed for the file and again for the total, including when a count is 0. Only R1 rows enter `modules` and `types`. The same module path or type from several files is added up. An absent or empty module is `(root)`. Rows are ordered by count descending, then by name ascending. `--top N` keeps the first N rows of each list. The rest is one final row `other`. When a real module or type is already named `other`, that remainder is `(other)` (then `(other 2)`, and so on) so every name stays unique. `--top 0` leaves only `other` when anything counted. Without `--top`, or when nothing remains, there is no remainder row.

`diff` compares addresses. Added lines are counting addresses in the new state that the old state does not have. Removed lines are counting addresses in the old state that the new state does not have. A changed rule is the same address with a different rule code, whether or not the instance counts. Those three lists are in address order. `rum delta` is new RUM minus old RUM, with a `+` when the number is positive. The text report does not repeat the module list. In JSON, `by_module` and `by_type` describe the new state.

Several files passed to `count` are summed. An address that also occurs in another of those files is not summed. The sum is not per hour and not per organization. "HCP Terraform combines workspace and Stacks resources under management when calculating costs". Stacks state is not supported. "Each partial hour is billed as a full hour". Rounding per hour is not modeled. The free plan with at most 500 managed resources is a limit, not a price.

The text always ends with `estimate: HCP Terraform bills the hourly peak; this is a single snapshot`. JSON uses the same sentence without the `estimate:` prefix.

## JSON output

`--json` prints one pretty-printed object and a final newline. `schema_version` is 1, `tool` is `rum-meter`, and `estimate` is the sentence above. A key that does not apply is left out. It is not written as null.

`count` has `inputs` (the paths, in order), `total_rum`, `by_module` and `by_type`. Each breakdown row is `name` and `count`. `instances` is present only with `--explain`. `costs` is present only with `--prices`.

`diff` has `inputs` as `old` and `new`, and `total_rum` as `old` and `new`. It also has `added`, `removed`, `changed_rule` (`address`, `old_rule`, `new_rule`), and `rum_delta`. `costs` is the new state's monthly estimate. `cost_delta` is the signed difference of the rounded cent amounts, in price-file order. Both are left out when there is no price file. `instances`, when `--explain` is set, lists the old file and then the new file.

An instance object has `file`, `address`, `counts` and `rule`. `deposed` is the key string when the object is deposed. `tainted` is `true` only when `status` is `tainted`.

Text reports and error messages escape control characters, and invisible or bidirectional format characters such as U+200B and U+202E, in addresses, file paths and price labels as `\uXXXX`. JSON preserves the original strings.

## Exit codes and usage errors

| Code | Meaning |
| --- | --- |
| 0 | Success. An equal threshold is included. |
| 1 | Threshold: summed RUM above `--max-rum` on count, or the RUM increase above `--max-increase` on diff. The report is still printed. |
| 2 | Input error: unreadable file, not a regular file, invalid JSON, a state that is not a JSON object, state version not 4, UTF-16 or UTF-32, a duplicate address, an invalid price file, or a cost that overflows. Stdout is empty. |
| 3 | Usage error, including a flag that would not change the report. Stdout is empty. |

A report that cannot be written or flushed to stdout also exits 2, with `rum-meter: cannot write report to stdout`. Some report bytes may already have been written when the failure occurs. A closed stdout is the same error. `--version` and `--help` that cannot be written or flushed also exit 2, not 0, with `rum-meter: cannot write message to stdout`. A failed write does not panic. When stderr cannot be written either, the exit code is unchanged and there is no panic.

Every row below is run against the binary. The message is the whole stderr, and `\n` is a newline.

| Option | Applies to | Case | Wrong use | Exit | Message |
| --- | --- | --- | --- | --- | --- |
| `--max-increase` | count | On count. | `count --max-increase 0 tests/fixtures/v4_one_managed.json` | 3 | `--max-increase only works with diff\n` |
| `--max-increase` | count | Wins over a later check. | `count --explain --hours-per-month 1 --max-increase 0 tests/fixtures/v4_one_managed.json` | 3 | `--max-increase only works with diff\n` |
| `--max-rum` | diff | On diff. | `diff --max-rum 0 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--max-rum only works with count\n` |
| `--max-rum` | diff | Wins over a later check. | `diff --explain --top 1 --hours-per-month 1 --max-rum 0 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--max-rum only works with count\n` |
| `--hours-per-month` | count | Without `--prices`. | `count --hours-per-month 1 tests/fixtures/v4_one_managed.json` | 3 | `--hours-per-month only works together with --prices\n` |
| `--hours-per-month` | diff | Without `--prices`. | `diff --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--hours-per-month only works together with --prices\n` |
| `--hours-per-month` | diff | `--json` does not change the message. | `diff --json --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--hours-per-month only works together with --prices\n` |
| `--hours-per-month` | diff | `--json` makes `--top` and `--explain` valid, so this check wins. | `diff --explain --top 1 --json --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--hours-per-month only works together with --prices\n` |
| `--top` | diff | Without `--json`. | `diff --top 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--top with diff only works together with --json\n` |
| `--top` | diff | Wins over `--explain`. | `diff --explain --top 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--top with diff only works together with --json\n` |
| `--explain` | diff | Without `--json`. | `diff --explain tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--explain with diff only works together with --json\n` |
| arguments | count | No state file. | `count` | 3 | `error: the following required arguments were not provided:\n  <STATE>...\n\nUsage: rum-meter count <STATE>...\n\nFor more information, try '--help'.\n` |
| arguments | diff | The new file is missing. | `diff only.json` | 3 | `error: the following required arguments were not provided:\n  <NEW>\n\nUsage: rum-meter diff <OLD> <NEW>\n\nFor more information, try '--help'.\n` |
| arguments | top level | Unknown flag. | `--not-an-option` | 3 | `error: unexpected argument '--not-an-option' found\n\nUsage: rum-meter [OPTIONS] <COMMAND>\n\nFor more information, try '--help'.\n` |
| `--top` | count | Not a number. | `count --top abc tests/fixtures/v4_one_managed.json` | 3 | `error: invalid value 'abc' for '--top <N>': invalid digit found in string\n\nFor more information, try '--help'.\n` |
| state file | count | Missing path. | `count tests/fixtures/missing.json` | 2 | `rum-meter: tests/fixtures/missing.json: unreadable (not found)\n` |
| state file | count | A directory is not a regular file. | `count tests/fixtures` | 2 | `rum-meter: tests/fixtures: not a regular file\n` |
| state file | count | Empty file. | `count tests/fixtures/empty.json` | 2 | `rum-meter: tests/fixtures/empty.json: invalid json at line 1 column 0\n` |
| state file | count | Truncated JSON. | `count tests/fixtures/broken.json` | 2 | `rum-meter: tests/fixtures/broken.json: invalid json at line 2 column 0\n` |
| state file | count | Version 3. | `count tests/fixtures/version_3.json` | 2 | `rum-meter: tests/fixtures/version_3.json: state version is not 4\n` |
| state file | count | Version is text, not a number. | `count tests/fixtures/version_string.json` | 2 | `rum-meter: tests/fixtures/version_string.json: invalid json at line 1 column 33\n` |
| `--prices` | count | A state file has no currency. | `count --prices tests/fixtures/v4_one_managed.json tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures/v4_one_managed.json: currency is empty\n` |
| `--prices` | count | The price path is a directory. | `count --prices tests/fixtures tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures: not a regular file\n` |
| `--prices` | count | The price file is missing. | `count --prices tests/fixtures/missing-prices.json tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures/missing-prices.json: unreadable (not found)\n` |
| `--prices` | count | The price file is empty. | `count --prices tests/fixtures/empty.json tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures/empty.json: invalid json at line 1 column 0\n` |
| `--prices` | count | Two tiers have the same name. | `count --prices tests/fixtures/dup-tier-prices.json tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures/dup-tier-prices.json: duplicate tier name\n` |
| state file | diff | The old state is version 3. | `diff tests/fixtures/version_3.json tests/fixtures/v4_handwritten.json` | 2 | `rum-meter: tests/fixtures/version_3.json: state version is not 4\n` |
| state file | count | A device is not a regular file (Linux and macOS). | `count /dev/null` | 2 | `rum-meter: /dev/null: not a regular file\n` |
| `--prices` | count | The price path is a device (Linux and macOS). | `count --prices /dev/null tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: /dev/null: not a regular file\n` |

## Privacy and safety

rum-meter is fully offline: no network, no API key, no telemetry. The binary reads only the paths you pass. Output contains addresses, module paths, types, rule codes and counts. Attribute values, sensitive values, private data and outputs are left out, because those fields are not deserialized. No error message contains bytes from the file, only the path, the line and column of a parse error, and the kind of error. `tests/safety.rs` guards that with synthetic fixtures. The dependencies are bigdecimal, clap, serde and serde_json.
