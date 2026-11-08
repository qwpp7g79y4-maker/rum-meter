# rum-meter count

Total RUM: 2

Not counted: R2 1, R3 2, R4 1

## Files

| File | RUM | Not counted |
| --- | --- | --- |
| tests/fixtures/v4\_handwritten.json | 2 | R2 1, R3 2, R4 1 |

## Modules

| Name | RUM |
| --- | --- |
| module.child | 2 |

## Types

| Name | RUM |
| --- | --- |
| random\_id | 2 |

## Costs

| Tier | Currency | Amount |
| --- | --- | --- |
| example-a | EUR | 1460.00 |
| example-b | EUR | 730.00 |

## Instances

| File | Address | Verdict | Rule | Tainted |
| --- | --- | --- | --- | --- |
| tests/fixtures/v4\_handwritten.json | null\_resource.counted\[0\] | does not count | R3 | no |
| tests/fixtures/v4\_handwritten.json | null\_resource.counted\[3\] | does not count | R3 | no |
| tests/fixtures/v4\_handwritten.json | module.child.random\_id.labeled\["web"\] | counts | R1 | no |
| tests/fixtures/v4\_handwritten.json | module.child.random\_id.labeled\["db"\] | counts | R1 | no |
| tests/fixtures/v4\_handwritten.json | module.child.module.nested.data.local\_file.cfg | does not count | R2 | no |
| tests/fixtures/v4\_handwritten.json | module.child.module.nested.terraform\_data.unused | does not count | R4 | no |

Estimate: HCP Terraform bills the hourly peak; this is a single snapshot
