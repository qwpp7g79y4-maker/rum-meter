# Examples

`one-managed.tfstate` is a fictional Terraform state of format version 4, written for this handbook. It is not a state file from this machine. It has one managed `random_id.handbook` instance.

Run this from the repository root:

```text
rum-meter count examples/one-managed.tfstate
```

Expected exit code: 0

## Expected output

Every file here is fictional. None is a state from a real workspace, and none holds a key, a token or an attribute value that means anything. Each command below exits 0 and prints exactly the matching `.txt` file. `tests/examples_golden.rs` compares them.

| Input | Command | Expected output |
|---|---|---|
| `one-managed.tfstate`: one managed `random_id` | `rum-meter count examples/one-managed.tfstate` | `one-managed.count.txt` |
| `web-stack.tfstate`: a VPC, a `web` module with two instances and a security group, a data source, a `null_resource` and a `terraform_data` | `rum-meter count --explain examples/web-stack.tfstate` | `web-stack.count.txt` |
| `web-stack.tfstate` and `web-stack-next.tfstate`: the same stack after one instance is removed and a `db` module is added | `rum-meter diff examples/web-stack.tfstate examples/web-stack-next.tfstate` | `web-stack.diff.txt` |
