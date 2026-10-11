#!/bin/sh
# Recorded commands for the OpenTofu fixtures. cargo test does not run this script.
set -eu

TOFU=${TOFU:-"$HOME/.local/bin/tofu"}
export TF_PLUGIN_CACHE_DIR="${TF_PLUGIN_CACHE_DIR:-/tmp/rum-meter-plugin-cache}"
export CHECKPOINT_DISABLE=1
export TF_IN_AUTOMATION=1
mkdir -p "$TF_PLUGIN_CACHE_DIR"

cd "$(CDPATH= cd -- "$(dirname "$0")" && pwd)"

# (a) count: random_id.servers count = 3, plus one random_password.secret.
(
  cd count
  timeout 300 "$TOFU" init -input=false
  timeout 300 "$TOFU" apply -auto-approve -input=false
)

# (b) for_each: random_id.labeled for web, db and api.
(
  cd for_each
  timeout 300 "$TOFU" init -input=false
  timeout 300 "$TOFU" apply -auto-approve -input=false
)

# (c) module: random_id.one in child, random_id.pair count = 2 in child/nested.
(
  cd module
  timeout 300 "$TOFU" init -input=false
  timeout 300 "$TOFU" apply -auto-approve -input=false
)

# (d) data: random_id.kept (R1), data.local_file.note (R2),
# null_resource.wait and terraform_data.marker (R3).
(
  cd data
  printf '%s\n' 'canary-data-local-file-content-7f3a9c2e1b04d8aa91c0ee' > note.generated
  timeout 300 "$TOFU" init -input=false
  timeout 300 "$TOFU" apply -auto-approve -input=false
)

# (e) deposed: one random_pet.replaced with create_before_destroy.
# The second apply is meant to fail: the previous object stays deposed.
(
  cd deposed
  touch allow-create.generated
  timeout 300 "$TOFU" init -input=false
  timeout 300 "$TOFU" apply -auto-approve -input=false
  rm -f allow-create.generated
  set +e
  timeout 300 "$TOFU" apply -auto-approve -input=false -var 'keeper=canary-deposed-keeper-second-7f3a9c2e1b04d8aa'
  status=$?
  set -e
  if [ "$status" -eq 0 ] || [ "$status" -eq 124 ]; then
    echo "deposed: the second apply had to fail (status $status)" >&2
    exit 1
  fi
)
