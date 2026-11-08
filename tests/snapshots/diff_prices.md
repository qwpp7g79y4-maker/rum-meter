# rum-meter diff

Old: tests/fixtures/v4\_diff\_old.json

New: tests/fixtures/v4\_diff\_new.json

RUM: 6 -\> 4

RUM delta: -2

## Added

| Address |
| --- |
| random\_id.servers\[1\] |

## Removed

| Address |
| --- |
| local\_file.note |
| module.child.random\_id.labeled\["gone"\] |
| random\_id.marker |

## Changed rule

none

## Cost delta

| Tier | Currency | Amount |
| --- | --- | --- |
| example-a | EUR | -1460.00 |
| example-b | EUR | -730.00 |

Estimate: HCP Terraform bills the hourly peak; this is a single snapshot
