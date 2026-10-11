# Fixtures

Every file under this directory that is kept for the tests is synthetic. None of them is a state from a real workspace, from GitHub, or from a customer. terraform is not installed. The JSON files were written by hand: no provider was downloaded and no `terraform init` or `terraform apply` was run for them.

Attribute values, `sensitive_attributes`, `private` and outputs in the JSON files are made-up markers for the tests. This page does not repeat those values, and it does not repeat values from the OpenTofu states. rum-meter does not read them into a type and does not print them.

`tests/edge_cases.rs` writes synthetic states and a synthetic price file in a temporary directory, including one state with 200000 instances, and deletes them afterwards.

## broken.json

Synthetic fragment, written by hand. The object starts as version 4 and is cut off, so it is not valid JSON. A marker sits in an extra field. `count` exits 2 and names the path, the line and the column, without bytes from the file.

## broken_canary.json

Synthetic invalid JSON, written by hand. It looks like a state: version 4, one managed `random_id.kept`, and the fields `attributes`, `sensitive_attributes`, `private` and `outputs`. The closing braces are missing, so it does not parse. `count` exits 2. The error text does not contain the markers.

## empty.json

Synthetic empty file, 0 bytes, written by hand. `count` exits 2: invalid JSON at line 1, column 0.

## example-prices.json

Synthetic price file, written by hand. It is not a state and not a HashiCorp rate. Two made-up tiers in EUR: `example-a` at 1 per RUM-hour and `example-b` at 0.5 per RUM-hour. The `note` is Dutch. The tool ignores `note` and does not print it.

## v4_canary_new.json

Synthetic state v4, written by hand. Resources: managed `random_id.kept` with index 0 and 1, data `local_file.note`, managed `null_resource.wait` with index 0, and managed `random_id.absent` in `module.child.module.nested` with 0 instances. Each present instance has `attributes`, `sensitive_attributes` and `private`, and there is an output. The identity fields do not hold the marker. Together with `v4_canary_old.json` this covers `count`, `--explain`, `--json` and `diff` in the safety test.

## v4_canary_old.json

Synthetic state v4, written by hand. Same kind of markers as `v4_canary_new.json`. Resources: managed `random_id.kept` with index 0, managed `random_id.labeled` in `module.child` with key `web`, managed `local_file.note`, managed `null_resource.wait` with index 0, and managed `random_id.absent` in `module.child.module.nested` with 0 instances.

## v4_diff_new.json

Synthetic state v4, written by hand. Counting instances: `random_id.servers[0]`, `random_id.servers[1]`, `random_id.steady` and `module.child.random_id.labeled["web"]`. Also: managed `terraform_data.marker` (does not count, R3), data `local_file.note` (address `data.local_file.note`, R2) and data `local_file.cfg` (address `data.local_file.cfg`, R2). In the older state, `marker` was a `random_id`.

## v4_diff_old.json

Synthetic state v4, written by hand. Counting instances: `random_id.servers[0]`, `random_id.steady`, `random_id.marker`, `local_file.note`, `module.child.random_id.labeled["web"]` and `["gone"]`. Data `local_file.cfg` does not count; the address is `data.local_file.cfg`. Against `v4_diff_new.json`: `servers[1]` is added, and `labeled["gone"]`, `random_id.marker` and managed `local_file.note` are removed. In the newer state the data source is `data.local_file.note`, a different address, so the rule does not change.

## v4_handwritten.json

Synthetic state v4, written by hand. Four resources:

- root, managed `null_resource.counted`, index 0 and 3 (`count`)
- `module.child`, managed `random_id.labeled`, keys `web` and `db` (`for_each`)
- `module.child.module.nested`, data `local_file.cfg`, 1 instance without an index. The address is `module.child.module.nested.data.local_file.cfg`
- `module.child.module.nested`, managed `terraform_data.unused`, 0 instances

Markers sit in attributes, sensitive_attributes, private and one output. The parser does not keep those fields.

## v4_object_status.json

Synthetic state v4, written by hand. Three managed `local_file` instances, 3 RUM together:

- `local_file.a`, the current object, without `status` and without `deposed`
- `local_file.a` with deposed key `00000001`. The address is `local_file.a (deposed 00000001)`
- `local_file.b` with `status` `tainted`. The address stays `local_file.b`; the tainted mark belongs to the explanation, not to the address

Markers sit in attributes, sensitive_attributes, private and one output. The deposed key is not a marker. This page does not repeat the markers.

## v4_one_managed.json

Synthetic state v4, written by hand. One managed `random_id.one` without `count` or `for_each`: 1 counting instance. There is an output with a marker.

## v4_top.json

Synthetic state v4, written by hand. Five module groups and five counting types, 16 RUM together.

Counting:

- `module.compute`: 5 (`random_id.ids` x2, `random_integer.n` x1, `local_file.files` x2)
- `module.network`: 5 (`random_id.net` x1, `random_pet.pets` x2, `local_file.files` x2)
- `module.app`: 3 (`random_integer.n` x2, `random_string.names` x1)
- `module.data`: 2 (`random_pet.pet` x1, `random_string.name` x1)
- `(root)`: 1 (`random_id.one`)

Not counting: data `local_file.cfg` in `module.app` (R2), `null_resource.skip` at the root with 2 instances and `terraform_data.marker` in `module.data` (R3), empty `random_id.gone` in `module.network` (R4). Counting types: `local_file` 4, `random_id` 4, `random_integer` 3, `random_pet` 3, `random_string` 2.

## version_3.json

Synthetic JSON, written by hand. `version` is 3 and `resources` is empty. One output contains a marker. rum-meter refuses it with exit 2 (`state version is not 4`) and does not put the marker in the message.

## version_string.json

Synthetic JSON, written by hand. `version` is text, a marker, instead of a number. The parser expects a number and exits 2 (invalid JSON, line and column). The message does not contain the text from the file. This page does not repeat the marker.

## tofu

Synthetic states produced by OpenTofu. OpenTofu v1.13.1, binary `~/.local/bin/tofu`. Providers only from registry.opentofu.org. `TF_PLUGIN_CACHE_DIR` is `/tmp/rum-meter-plugin-cache` (not in git). `tests/fixtures/tofu/generate.sh` sets that variable and, per scenario, runs `timeout 300 tofu init -input=false` and `timeout 300 tofu apply -auto-approve -input=false`. `cargo test` does not run the script and does not need `tofu`.

`.terraform/`, the lock file, backups and `*.generated` are not in git.

### count

Synthetic. Directory `tests/fixtures/tofu/count/`. Files: `tofu/count/main.tf` and `tofu/count/terraform.tfstate`. Provider hashicorp/random 3.9.1. `main.tf`: `random_id.servers` with `count = 3`, and one `random_password.secret` (`length` 32, sensitive `result`). 4 managed instances, 0 data sources, 0 `null_resource`, 0 `terraform_data`, 0 empty blocks. Expected from `main.tf`: 4 RUM, R2 0, R3 0, R4 0.

### for_each

Synthetic. Directory `tests/fixtures/tofu/for_each/`. Files: `tofu/for_each/main.tf` and `tofu/for_each/terraform.tfstate`. Provider hashicorp/random 3.9.1. `main.tf`: `random_id.labeled` with `for_each` over `web`, `db` and `api`. 3 managed instances and nothing else. Expected from `main.tf`: 3 RUM, R2 0, R3 0, R4 0.

### module

Synthetic. Directory `tests/fixtures/tofu/module/`. Files: `tofu/module/main.tf`, `tofu/module/terraform.tfstate`, `tofu/module/child/main.tf` and `tofu/module/child/nested/main.tf`. Provider hashicorp/random 3.9.1. `main.tf` calls `module.child`. That module has one `random_id.one` and calls `module.nested`. The nested module has `random_id.pair` with `count = 2`. 3 managed instances and nothing else. Expected from `main.tf`: 3 RUM, R2 0, R3 0, R4 0. Addresses: `module.child.random_id.one`, `module.child.module.nested.random_id.pair[0]` and `[1]`.

### data

Synthetic. Directory `tests/fixtures/tofu/data/`. Files: `tofu/data/main.tf` and `tofu/data/terraform.tfstate`. Providers: hashicorp/local 2.9.1, hashicorp/null 3.3.2, hashicorp/random 3.9.1. `terraform_data` is built in. `main.tf`: one managed `random_id.kept`, data source `local_file.note` (reads `note.generated`), one `null_resource.wait` and one `terraform_data.marker`. `generate.sh` writes `note.generated` before `init`. That file is not in git. Expected from `main.tf`: 1 RUM, R2 1, R3 2, R4 0.

### deposed

Synthetic. Directory `tests/fixtures/tofu/deposed/`. Files: `tofu/deposed/main.tf` and `tofu/deposed/terraform.tfstate`. Provider hashicorp/random 3.9.1. `main.tf`: one `random_pet.replaced` with `lifecycle.create_before_destroy`. A destroy-time `local-exec` does not leave the old object deposed here: when OpenTofu 1.13.1 destroys a deposed object it calls only the provider delete and skips that provisioner. `generate.sh` therefore runs two applies. The first creates `allow-create.generated` (not in git) and succeeds. The second removes that file, changes `keeper` with `-var`, and fails in the create-time `local-exec` after the previous object is already deposed. That apply does not exit 0. Expected from `main.tf` plus that failed replace: the current object and the deposed object, 2 RUM, R2 0, R3 0, R4 0. The current object is tainted. The literal `-var` is in `tofu/generate.sh`. This page does not repeat it.
