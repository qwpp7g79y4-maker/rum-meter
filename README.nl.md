# rum-meter

[English](README.md) · Nederlands

Tel factureerbare HCP Terraform-resources in een lokaal statebestand, vóór de factuur.

[![crates.io](https://img.shields.io/crates/v/rum-meter.svg)](https://crates.io/crates/rum-meter) [![CI](https://github.com/qwpp7g79y4-maker/rum-meter/actions/workflows/ci.yml/badge.svg)](https://github.com/qwpp7g79y4-maker/rum-meter/actions/workflows/ci.yml)

[Installatie](#installatie) • [Snel aan de slag](#snel-aan-de-slag) • [Hoe het rekent](#hoe-het-rekent) • [Alternatieven](#vergelijking-met-alternatieven) • [Beperkingen](#beperkingen) • [Referentie](docs/reference.nl.md)

rum-meter is een opdrachtregelprogramma in Rust. Het leest `terraform.tfstate`-bestanden van formaatversie 4 en telt factureerbare HCP Terraform Resources Under Management (RUM) volgens de definitie van HashiCorp. Het legt elke instantie uit, splitst het totaal uit per module en per type, schat maandkosten uit een prijsbestand dat je zelf meegeeft, en vergelijkt twee states. Het heeft geen account nodig en maakt geen netwerkverbinding. Het resultaat is een schatting.

De uitvoer van de tool is Engels; de voorbeelden hieronder tonen die precies zoals de binary hem schrijft.

## Waarom

HCP Terraform rekent per RUM af. Een team ziet die telling op de billingpagina, achteraf, zonder uitleg per resource. rum-meter laat vooraf, op de eigen machine, zien hoeveel RUM een state heeft, in welke module en welk type dat zit, en wat een wijziging toevoegt of weghaalt. Betaalbereidheid is niet onderzocht. De zinnen van HashiCorp waar dit op steunt zijn nagelezen op 7 oktober 2026 en staan onder [Vergelijking met alternatieven](#vergelijking-met-alternatieven).

## Installatie

Met Rust 1.85 of nieuwer installeer je vanaf [crates.io](https://crates.io/crates/rum-meter):

```sh
cargo install --locked rum-meter
rum-meter --help
```

Kant-en-klare binaries voor Linux (x86_64, glibc en musl), Windows (x86_64) en macOS (Intel en Apple Silicon), elk met een SHA-256-checksum, staan op de [Releases-pagina](https://github.com/qwpp7g79y4-maker/rum-meter/releases). Pak het archief uit en zet `rum-meter` in je `PATH`.

Voor de nieuwste `main`: `cargo install --locked --git https://github.com/qwpp7g79y4-maker/rum-meter`.

Vanuit een lokale clone: `cargo install --locked --path .`, of `cargo build --release`; de binary is `target/release/rum-meter`.

## Systeemvereisten

| Platform | Release-archief | Vereiste |
|---|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | glibc 2.34 of nieuwer (bijvoorbeeld Ubuntu 22.04, Debian 12, RHEL 9) |
| Linux x86_64 | `x86_64-unknown-linux-musl` | geen; statisch gelinkt, ook voor Alpine en oudere distributies |
| macOS Apple Silicon | `aarch64-apple-darwin` | macOS 11 of nieuwer |
| macOS Intel | `x86_64-apple-darwin` | macOS 10.12 of nieuwer |
| Windows x86_64 | `x86_64-pc-windows-msvc` | Windows 10 of nieuwer |

De binaries worden gebouwd en met een rooktest gecontroleerd op runners van GitHub (Ubuntu, macOS op Apple Silicon en Intel, Windows). Bouwen vanuit de broncode vraagt Rust 1.85 of nieuwer op elk platform dat Rust ondersteunt.

## Snel aan de slag

Dit is de echte uitvoer van de binary op de synthetische fixtures in `tests/fixtures`. Een test draait elk voorbeeld en vergelijkt de uitvoer.

Eén state tellen:

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

`--explain` zet onder het bestand één regel per instantie: het adres, `counts` of `does not count`, en de regelcode. Een tainted object krijgt er ` tainted` bij.

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

Een ontbrekende module is `(root)`. Rijen staan op aantal aflopend, en bij een gelijk aantal op naam oplopend. `--top N` houdt van elke lijst de eerste N rijen. De rest is één rij `other`.

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

`--json` vervangt de tekst door één object. `schema_version` is 1.

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

Kosten komen alleen mee met `--prices`. Het prijsbestand is invoer: een valuta, en per tier een naam en een prijs per RUM-uur. Een maand is RUM maal die prijs maal het aantal uren. Standaard is dat 730 uur. `--hours-per-month` zet een ander aantal. Op de pagina 'Estimate HCP Terraform cost' rekent het voorbeeld van HashiCorp met "Per Hour Price x 24 x 30" (720 uur). rum-meter gebruikt standaard 730, en `--hours-per-month 720` volgt dat voorbeeld. `tests/fixtures/example-prices.json` is een verzonnen voorbeeld, geen tarief van HashiCorp. De `note` in dat bestand is Nederlands. De tool negeert `note` en drukt die niet af.

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

Eén RUM op dat voorbeeld, 730 uur, is 730.00 en 365.00. Die bedragen volgen uit het voorbeeldbestand, niet uit een prijslijst van HashiCorp. Een komma is geen decimaalteken (`0.5`, niet `0,5`).

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

Hetzelfde bestand met 720 uur:

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

`diff` vergelijkt twee states: toegevoegde en verwijderde tellende adressen, instanties die van regel wisselen, en het RUM-verschil. Met `--prices` staat er ook het maandelijkse kostenverschil per tier.

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

Exitcode 0 is gelukt. Exitcode 1 is een drempel: `--max-rum` bij `count`, `--max-increase` bij `diff`. Het rapport wordt toch afgedrukt. Exitcode 2 is een invoerfout (onleesbaar bestand, geen gewoon bestand, ongeldige JSON, een state die geen JSON-object is, stateversie niet 4, een dubbel adres, UTF-16 of UTF-32, een ongeldig prijsbestand, of kosten die overlopen). Exitcode 3 is een gebruiksfout. Een vlag die het rapport niet zou veranderen is ook exit 3, met een vaste melding op stderr en niets op stdout, vóór het lezen van de states. Staan er meer tegelijk, dan wint de eerste van deze:

- `--max-increase only works with diff`
- `--max-rum only works with count`
- `--hours-per-month only works together with --prices`
- `--top with diff only works together with --json`
- `--explain with diff only works together with --json`

Een rapport dat niet naar stdout geschreven kan worden is exit 2, met `rum-meter: cannot write report to stdout`, en een gesloten stdout is dezelfde fout. `--version` en `--help` die niet geschreven kunnen worden zijn exit 2, niet 0, met `rum-meter: cannot write message to stdout`. Een mislukte schrijfactie geeft geen panic, ook niet als stderr niet geschreven kan worden.

Alle opties, exitcodes en foutmeldingen staan in de [referentie](docs/reference.nl.md) en in `rum-meter --help`.

## Hoe het rekent

Elk getal is terug te voeren op deze regels. Het adresformaat, het prijsbestand en het JSON-object staan in de [referentie](docs/reference.nl.md#hoe-het-rekent).

- **R1 telt:** een resource met `mode` `managed`, ook een resource die alleen lokaal bestaat. Bij `count` en `for_each` telt elke instantie. Een tainted object telt onder R1 en houdt hetzelfde adres. `--explain` en JSON markeren het als tainted.
- **R2 telt niet:** `mode` `data` (een data source is geen managed resource), met de reden `data source`. Aanname: een onbekende mode (niet `managed` en niet `data`) is geen managed resource, telt niet, houdt R2 en krijgt de reden `unknown mode`.
- **R3 telt niet:** type `null_resource` of `terraform_data` ("Null Resources" zijn uitgezonderd). Aanname: `terraform_data` valt onder "Null Resources".
- **R4:** een resourceblok zonder instanties telt 0.
- **Deposed:** een deposed object is een eigen infrastructuurobject. Het adres eindigt op ` (deposed KEY)`, bijvoorbeeld `local_file.a (deposed 00000001)`. Een tellend deposed object blijft op R1. Een deposed `null_resource` of `terraform_data` blijft op R3. Er is geen aparte regelcode voor.
- **Meerdere bestanden** worden opgeteld. Alleen tellende instanties (R1) komen in de module- en typelijsten. Hetzelfde modulepad of type uit meerdere states wordt opgeteld. Hetzelfde adres in meer dan één van die bestanden wordt niet opgeteld: dat is exit 2, `duplicate address`, met het adres. Hetzelfde bestand twee keer meegeven is die fout.
- **Aandeel van een lijst:** rijen staan op aantal aflopend, dan op naam oplopend. `--top N` houdt de eerste N rijen. De rest is één laatste rij `other`. Zonder `--top`, of als er niets overblijft, is er geen `other`-rij. Een ontbrekende of lege module is `(root)`.
- **Kosten:** per maand = RUM x prijs per RUM-uur x uren. Standaard 730 uur. De prijs is het exacte decimale getal uit het bestand. 1 RUM tegen 0.004999999999999999999 voor 1 uur is 0.00. 1 RUM tegen 1.005 voor 1 uur is 1.01. Bedragen worden half away from zero afgerond op 2 decimalen, en het decimaalteken in de uitvoer is een punt. `diff` trekt die afgeronde centbedragen van elkaar af. Een product dat overloopt is exit 2, nooit een afgekapt bedrag.
- **Schatting:** de tekst eindigt altijd op `estimate: HCP Terraform bills the hourly peak; this is a single snapshot`. JSON gebruikt dezelfde zin zonder het voorvoegsel `estimate:`.

Voorbeeld (de snelstart hierboven): 1 RUM tegen 1 per RUM-uur voor 730 uur is 730.00. Tegen 0.5 per RUM-uur is dat 365.00. Met `--hours-per-month 720` zijn dezelfde prijzen 720.00 en 360.00.

## Vergelijking met alternatieven

Een eerlijke vergelijking met de eigen billing van HCP Terraform, de definitie die rum-meter volgt. De pagina's zijn nagelezen op 7 oktober 2026. Wat die pagina's niet zeggen, staat hier als niet geverifieerd.

**HCP Terraform** (https://developer.hashicorp.com/hcp/docs/hcp/admin/billing/pricing-definitions) definieert wat meetelt als resource under management. De zin op die pagina, "Each infrastructure object, including but not limited to “Local Only” Resources, counts as a single Resource.", is de basis om elk object te tellen, ook een object dat alleen lokaal bestaat. Dezelfde pagina is de bron voor "Hourly Peak Managed Resources": de factuur gebruikt het maximum aantal gelijktijdige managed resources in een billing-uur, opgeteld over de HCP Terraform-organisaties onder één HCP-organisatie. rum-meter kan die piek niet zien. Het telt één momentopname en zegt dat altijd.

https://developer.hashicorp.com/terraform/cloud-docs/overview zegt "A managed resource is a resource in an HCP Terraform-managed state file where `mode = "managed"`. HCP Terraform counts a resource as part of this count starting from the first `terraform plan` or `terraform apply` operation on the resource." rum-meter ziet die historie niet. Het neemt elke instantie in het bestand dat je meegeeft. Of een resource in dat bestand een plan of apply heeft gehad, is niet geverifieerd.

https://developer.hashicorp.com/terraform/cloud-docs/workspaces/state zegt "HCP Terraform includes resources in modules and each resource instance created with the `count` or `for_each` meta-arguments." rum-meter doet hetzelfde voor een statebestand op schijf.

Op de pagina 'Estimate HCP Terraform cost' is het rekenvoorbeeld "Per Hour Price x 24 x 30" (720 uur). De standaardmaand van rum-meter is 730 uur. Geef `--hours-per-month 720` voor dat voorbeeld. De prijzen van een prijslijst van HashiCorp zitten niet in deze tool. Of de huidige lijst dat voorbeeld van 720 uur nog gebruikt, is niet geverifieerd buiten de zin die hier staat.

Terraform schrijft per object in state v4 `index_key`, `status` (alleen `tainted`) en `deposed` (`internal/states/statefile/version4.go`, commit b6daf8bc, regels 705-707). Aanname, omdat het geen aparte zin op de pricingpagina is: een deposed object telt als een eigen infrastructuurobject. Die lezing is niet geverifieerd als billingregel van HashiCorp. Ze volgt de zin over elk infrastructuurobject.

De billingpagina toont de telling achteraf. rum-meter leest een bestand dat je al hebt, legt elke instantie uit met een regelcode, en stuurt de state nergens heen. Het vervangt de factuur niet.

## Beperkingen

- De officiële afrekening is de hourly peak: het maximum aantal gelijktijdige managed resources per billing-uur, opgeteld over de HCP Terraform-organisaties onder één HCP-organisatie. Die piek is offline niet te meten. rum-meter telt één momentopname per bestand en noemt de uitkomst een schatting.
- De definitie telt een resource als die in een door HCP Terraform beheerde state staat en minstens één plan of apply heeft gehad. rum-meter ziet die historie niet en neemt elke instantie in het gegeven bestand mee.
- Alleen stateformaat v4 (Terraform 0.12 en nieuwer) wordt gelezen, en de wortel moet een JSON-object zijn. Een JSON-array is exit 2, `state is not an object`. Remote state en planbestanden blijven buiten de tool. Tel meerdere workspaces door meerdere bestanden mee te geven. De som is niet per uur en niet per organisatie. Een adres dat ook in een ander van die bestanden staat is exit 2, `duplicate address`, en wordt niet twee keer geteld.
- Eén workspace is één state. Flex, contracten en andere kortingen komen alleen mee als ze in het prijsbestand staan. rum-meter controleert die getallen niet. Oude free-tier-regels en Terraform Enterprise-licenties hebben geen eigen model.
- "Each partial hour is billed as a full hour". Afronden per uur is niet gemodelleerd.
- "HCP Terraform combines workspace and Stacks resources under management when calculating costs". Stacks-state wordt niet ondersteund.
- Het gratis plan met maximaal 500 managed resources is een limiet, geen prijs.
- Een state hoort elk adres één keer te hebben. Komt een adres twee keer voor, op root, in een module, met count of for_each, of als deposed object, dan telt rum-meter die state niet: exit 2, met het adres en de reden, zonder attribuutwaarden. Dezelfde regel geldt als `count` meerdere bestanden krijgt: het bestand dat een adres herhaalt is exit 2, en de totalen worden niet afgedrukt. Bij `diff` blijven added en removed daardoor optellen tot de RUM-delta.
- rum-meter leest UTF-8, voor statebestanden en prijsbestanden. Een UTF-8 BOM aan het begin van het bestand wordt overgeslagen. Een UTF-16- of UTF-32-bestand, met of zonder BOM, zoals Windows PowerShell 5.1 bij een redirect schrijft, wordt niet omgezet. Dat is exit 2, met de melding dat het bestand UTF-16 (of UTF-32) is en als UTF-8 opgeslagen moet worden, bijvoorbeeld met `iconv -f UTF-16 -t UTF-8` of in PowerShell met `Out-File -Encoding utf8`. CRLF-regeleinden zijn geen probleem.
- Een bestandsnaam die geen geldige UTF-8 is (kan op Linux) wordt wel gelezen, maar in het rapport getoond met U+FFFD voor de ongeldige bytes.
- rum-meter leest alleen gewone bestanden, dus geen pipe of process substitution. Een map, een FIFO of een apparaat wordt geweigerd met exit 2 voordat er een byte wordt gelezen. Een symlink naar een FIFO wordt op dezelfde manier geweigerd. Het openen wacht niet, en het type komt van het geopende bestand, zodat een pad dat een FIFO wordt niet wordt gelezen en niet blijft hangen. Een symlink naar een gewoon bestand wordt wel gelezen.

Waar de fixtures vandaan komen: [tests/fixtures/README.md](tests/fixtures/README.md).

## Privacy

rum-meter werkt volledig offline: geen netwerk, geen API-sleutel, geen telemetrie. De binary leest alleen de paden die je opgeeft. In de uitvoer staan adressen, modulepaden, types, regelcodes en aantallen. Attribuutwaarden, sensitive values, private data en outputs blijven achterwege. `tests/safety.rs` bewaakt dat met synthetische fixtures. Geen foutmelding bevat bytes uit het bestand, alleen het pad, de regel en de kolom van een parsefout, en het soort fout.

## Steunen

rum-meter is gratis te gebruiken. Bespaart het je geld, dan kun je het werk steunen via [Ko-fi](https://ko-fi.com/pepai).

## Licentie

Naar keuze onder een van deze licenties:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

Copyright 2026 PEPAI.

## Contact

PEPAI, pep_ai@icloud.com

Meld bugs en ideeën via [Issues](https://github.com/qwpp7g79y4-maker/rum-meter/issues); meld beveiligingsproblemen privé zoals beschreven in [SECURITY.md](SECURITY.md).
