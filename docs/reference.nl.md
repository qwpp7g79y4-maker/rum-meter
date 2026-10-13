# rum-meter referentie

[English](reference.md) · Nederlands

Deze pagina heeft alle details. Voor een overzicht, zie de [README](../README.nl.md).

De uitvoer van de tool is Engels, ook de meldingen in de tabel hieronder.

## Opties

Draai `rum-meter --help`, `rum-meter count --help` of `rum-meter diff --help` voor de ingebouwde hulp. `count` neemt een of meer statebestanden (`STATE`). `diff` neemt het eerdere bestand (`OLD`) en daarna het latere bestand (`NEW`).

Een vlag die het rapport niet zou veranderen is exit 3, met één vaste regel op stderr en niets op stdout, vóór er een bestand wordt gelezen. Staan er meer tegelijk, dan wint de eerste: `--max-increase` bij count, `--max-rum` bij diff, `--hours-per-month` zonder `--prices`, en bij diff zonder `--json` eerst `--top` en dan `--explain`. `--json` verandert een weigering niet. Mét `--json` doen `--top` en `--explain` bij diff wél iets, en eindigt het commando met exit 0. Zie [Exitcodes en gebruiksfouten](#exitcodes-en-gebruiksfouten).

| Optie | Werkt bij | Wat het doet |
| --- | --- | --- |
| `--json` | count en diff | Eén JSON-object in plaats van tekst. `schema_version` is 1. |
| `--explain` | count; bij diff alleen samen met `--json` | Eén regel per instantie: het adres, `counts` of `does not count`, en de regelcode. Een tainted object krijgt er ` tainted` bij. |
| `--top` | count; bij diff alleen samen met `--json` | Hoogstens N modulerijen en N typerijen. De rest is één rij `other`. |
| `--prices` | count en diff | Lees tierprijzen uit dit JSON-bestand. Zonder dit bestand is er geen kostenregel. |
| `--hours-per-month` | count en diff, alleen samen met `--prices` | Uren in de maand voor de schatting. Standaard 730. |
| `--max-rum` | count | Exit 1 als de opgetelde RUM boven N ligt. N zelf valt binnen de grens. Het rapport wordt toch afgedrukt. |
| `--max-increase` | diff | Exit 1 als nieuwe RUM min oude RUM boven N ligt. Een gelijke delta valt binnen de grens. Een daling valt binnen elke grens. Het rapport wordt toch afgedrukt. |
| `--help` | overal | Toon de hulp en exit 0. `-h` is de korte vorm. |
| `--version` | alleen het topniveau | Toon `rum-meter 0.1.0` en exit 0. `-V` is de korte vorm. |

## Prijsbestand

Prijzen komen alleen uit een JSON-bestand dat je met `--prices` meegeeft. Het programma heeft geen ingebouwde prijzen. Dit is de synthetische fixture `tests/fixtures/example-prices.json`:

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

`currency` is een niet-lege tekst. `tiers` is een niet-lege lijst van objecten. Elk object heeft een niet-lege `name` en een `price_per_rum_hour` die een eindig getal is, nul of hoger. `0` is een geldige prijs. Elk ander veld, ook `note`, wordt genegeerd en niet afgedrukt. De note in het voorbeeld is Nederlands. Het bestand is een voorbeeld en geen tarief van HashiCorp.

Een komma is geen decimaalteken (`0.5`, not `0,5`). Een `0,5` zonder aanhalingstekens is ongeldige JSON. Een `"0,5"` tussen aanhalingstekens is `price is not a number`.

Andere fouten in het prijsbestand, elk één regel op stderr en niets op stdout:

- geen object, bijvoorbeeld een JSON-array: `invalid price file`
- valuta ontbreekt of is leeg: `currency is empty`
- `tiers` ontbreekt, is geen lijst, of is een lege lijst: `price file has no tiers`
- een tier die geen object is: `tier is not an object`
- een lege tiernaam: `tier name is empty`
- een prijs die ontbreekt, geen getal is, of niet eindig is: `price is not a number`
- een negatieve prijs: `price is negative`
- het pad is geen gewoon bestand: `not a regular file`
- het pad gaat niet open: `unreadable (not found)`, `unreadable (permission denied)`, `unreadable (is a directory)` of `unreadable (io error)`
- de bytes zijn geen JSON: `invalid json at line L column C`

Een maand is RUM maal de prijs per RUM-uur maal de uren. Standaard is dat 730 uur. De prijs is het exacte decimale getal uit het bestand, dus 1 RUM tegen `0.004999999999999999999` voor 1 uur is `0.00`. De berekening gebruikt decimale rekenkunde: 1 RUM tegen 1.005 voor 1 uur is `1.01`. Het product wordt met 100 vermenigvuldigd en half away from zero afgerond op een heel aantal centen, en daarna getoond als euro's, een punt, en twee cijfers. `diff` trekt die centgehelen van elkaar af. Een positieve kostendelta krijgt een `+`, een negatieve een `-`, en nul heeft geen teken. 0 uur geeft `0.00`. 1 RUM tegen 0.125 voor 1 uur is `0.13`. 1 RUM tegen 0.0625 voor 1 uur is `0.06`.

Op het voorbeeldbestand is 1 RUM voor 730 uur `730.00` en `365.00`. Voor 720 uur is dat `720.00` en `360.00`. Die bedragen volgen uit het voorbeeldbestand, niet uit een prijslijst van HashiCorp.

Op de pagina 'Estimate HCP Terraform cost' is het rekenvoorbeeld "Per Hour Price x 24 x 30" (720 uur). Geef `--hours-per-month 720` voor dat voorbeeld. Of de huidige lijst dat voorbeeld van 720 uur nog gebruikt, is niet geverifieerd buiten de zin die hier staat.

Een product dat niet eindig is, negatief is, of niet in het centbedrag past, is exit 2: `monthly cost overflows`. Het bedrag wordt nooit afgekapt. Een invoerfout wordt afgedrukt vóór elk rapport, en wint dus van een drempel.

## Statebestand

Alleen stateformaat versie 4 wordt aanvaard, en `version` moet een getal zijn. De wortel moet een JSON-object zijn. Een JSON-array is `state is not an object`. Een tekst is `invalid json at line L column C`. Elk ander getal is `state version is not 4`.

De lezer bewaart `version`, en per resource `mode`, `type`, `name`, `module` en `instances`. Per instantie bewaart hij `index_key` (een getal of een tekst), de `deposed`-sleutel, en of `status` precies de tekst `tainted` is. Elke andere status vervalt. Onbekende JSON-velden worden genegeerd. `attributes`, `sensitive_attributes`, `private` en `outputs` worden niet gedeserialiseerd en nooit bewaard.

Het bestand moet UTF-8 zijn; dat geldt ook voor een prijsbestand. Een UTF-8 BOM vooraan (de 3 bytes `EF BB BF`) wordt overgeslagen. Een UTF-16 BOM (`FF FE` of `FE FF`), zoals Windows PowerShell 5.1 bij een redirect schrijft, wordt niet omgezet. Dat is exit 2: `file is UTF-16; save it as UTF-8`. Een UTF-32 BOM (`FF FE 00 00` of `00 00 FE FF`) geeft `file is UTF-32; save it as UTF-8`. Zonder BOM wordt de codering herkend aan de NUL-bytes in de eerste 4 bytes, zoals in RFC 4627: een JSON-tekst begint met een ASCII-teken. Niets van zo'n bestand wordt gedecodeerd. CRLF-regeleinden tellen hetzelfde als LF. Een pad dat geen geldige UTF-8 is wordt gelezen en getoond met U+FFFD voor de ongeldige bytes, in tekst en in JSON.

Alleen een gewoon bestand wordt gelezen, voor een state en voor een prijsbestand. Het bestand wordt geopend zonder te wachten, en het type komt van dat geopende bestand, vóór er een byte van wordt gelezen. Een map, een FIFO, een apparaat of een symlink daarnaartoe is `not a regular file`. Een pad dat een FIFO wordt, wordt niet gelezen en blijft niet hangen. Een symlink naar een gewoon bestand wordt wel gelezen. Daarom werken een pipe en process substitution niet. Een ontbrekend pad is `unreadable (not found)`. De andere onleesbare soorten zijn `permission denied`, `is a directory` en `io error`.

Elk adres mag één keer voorkomen, in één bestand en over de bestanden die aan `count` worden meegegeven. Een herhaling, op root, in een module, met count of for_each, als deposed object, of in een later bestand, is exit 2: `duplicate address`, plus het adres. Stuurtekens in dat adres worden geschreven als `\u00XX`. Attribuutwaarden zitten er niet bij. De eerste herhaling in de volgorde van de beslissingen wordt gemeld. Bij `diff` blijven added en removed daardoor optellen tot de RUM-delta. Tekstuitvoer escapt stuurtekens op dezelfde manier. JSON houdt de tekens en laat de JSON-encoder ze escapen.

## Hoe het rekent

De pagina's hieronder zijn nagelezen op 7 oktober 2026. Wat ze niet zeggen, staat als niet geverifieerd.

https://developer.hashicorp.com/hcp/docs/hcp/admin/billing/pricing-definitions zegt "Each infrastructure object, including but not limited to “Local Only” Resources, counts as a single Resource." Die zin is de basis om elk object te tellen, ook een object dat alleen lokaal bestaat. Dezelfde pagina is de bron voor "Hourly Peak Managed Resources": de factuur gebruikt het maximum aantal gelijktijdige managed resources in een billing-uur, opgeteld over de HCP Terraform-organisaties onder één HCP-organisatie. rum-meter kan die piek niet zien. Het telt één momentopname en zegt dat altijd.

https://developer.hashicorp.com/terraform/cloud-docs/overview zegt "A managed resource is a resource in an HCP Terraform-managed state file where `mode = "managed"`. HCP Terraform counts a resource as part of this count starting from the first `terraform plan` or `terraform apply` operation on the resource." rum-meter ziet die historie niet. Het neemt elke instantie in het bestand dat je meegeeft. Of een resource in dat bestand een plan of apply heeft gehad, is niet geverifieerd.

https://developer.hashicorp.com/terraform/cloud-docs/workspaces/state zegt "HCP Terraform includes resources in modules and each resource instance created with the `count` or `for_each` meta-arguments." rum-meter doet hetzelfde voor een statebestand op schijf.

Voorrang voor één resourceblok:

- Geen instanties: R4. Het telt 0. De classificatie-reden is `resource has no instances`.
- Type `null_resource` of `terraform_data`: R3, ook als `mode` `data` is. De reden is `null resource`. Aanname: `terraform_data` valt onder "Null Resources".
- `mode` `managed`: R1. Het telt. De reden is `managed resource`. Een resource die alleen lokaal bestaat, telt ook.
- `mode` `data`: R2. Het telt niet. De reden is `data source`.
- Elke andere mode: R2. Het telt niet. Aanname: een onbekende mode (niet `managed` en niet `data`) is geen managed resource, telt niet, houdt R2 en krijgt de reden `unknown mode`.

De reden is een classificatie-label. Het tekstrapport drukt hem niet af. Een explain-regel is het adres, dan `counts` of `does not count`, dan de regelcode (`R1`, `R2`, `R3` of `R4`). Een tainted object krijgt er ` tainted` bij. JSON `rule` is diezelfde code. Er is geen reden-veld. `tainted` staat er alleen als het `true` is. `deposed` staat er alleen als het object een deposed-sleutel heeft.

Bij `count` en `for_each` is elke instantie een eigen rij. Het adres is `module.a.module.b.type.name`, plus `[n]` voor een count-index of `["key"]` voor een for_each-sleutel. Een `"` of `\` in de sleutel wordt met een backslash geëscapt. Een data source zet `data.` vóór het type, dus `local_file.a` en `data.local_file.a` zijn verschillende adressen. Een lege module blijft van het adres af. Een niet-lege deposed-sleutel komt erachter als ` (deposed KEY)`, bijvoorbeeld `local_file.a (deposed 00000001)`. Een lege deposed-tekst wordt genegeerd. Een tellend deposed object blijft op R1. Een deposed null-type blijft op R3. Er is geen aparte regelcode voor.

Terraform schrijft per object in state v4 `index_key`, `status` (alleen `tainted`) en `deposed` (`internal/states/statefile/version4.go`, commit b6daf8bc, regels 705-707). Aanname, omdat het geen aparte zin op de pricingpagina is: een deposed object telt als een eigen infrastructuurobject. Die lezing is niet geverifieerd als billingregel van HashiCorp. Ze volgt de zin over elk infrastructuurobject.

De regel `not counted: R2 n, R3 n, R4 n` wordt voor het bestand afgedrukt en nog eens voor het totaal, ook als een aantal 0 is. Alleen R1-rijen komen in `modules` en `types`. Hetzelfde modulepad of type uit meerdere bestanden wordt opgeteld. Een ontbrekende of lege module is `(root)`. Rijen staan op aantal aflopend, dan op naam oplopend. `--top N` houdt van elke lijst de eerste N rijen. De rest is één laatste rij `other`. `--top 0` laat alleen `other` over als er iets telde. Zonder `--top`, of als er niets overblijft, is er geen `other`-rij.

`diff` vergelijkt adressen. Added zijn tellende adressen in de nieuwe state die de oude state niet heeft. Removed zijn tellende adressen in de oude state die de nieuwe state niet heeft. Een gewijzigde regel is hetzelfde adres met een andere regelcode, of de instantie nu telt of niet. Die drie lijsten staan op adresvolgorde. `rum delta` is nieuwe RUM min oude RUM, met een `+` als het getal positief is. Het tekstrapport herhaalt de modulelijst niet. In JSON beschrijven `by_module` en `by_type` de nieuwe state.

Meerdere bestanden aan `count` worden opgeteld. Een adres dat ook in een ander van die bestanden staat, wordt niet opgeteld. De som is niet per uur en niet per organisatie. "HCP Terraform combines workspace and Stacks resources under management when calculating costs". Stacks-state wordt niet ondersteund. "Each partial hour is billed as a full hour". Afronden per uur is niet gemodelleerd. Het gratis plan met maximaal 500 managed resources is een limiet, geen prijs.

De tekst eindigt altijd op `estimate: HCP Terraform bills the hourly peak; this is a single snapshot`. JSON gebruikt dezelfde zin zonder het voorvoegsel `estimate:`.

## JSON-uitvoer

`--json` drukt één pretty-printed object af en een laatste newline. `schema_version` is 1, `tool` is `rum-meter`, en `estimate` is de zin hierboven. Een sleutel die niet van toepassing is, blijft weg. Hij wordt niet als null geschreven.

`count` heeft `inputs` (de paden, op volgorde), `total_rum`, `by_module` en `by_type`. Elke uitsplitsingsrij is `name` en `count`. `instances` staat er alleen met `--explain`. `costs` staat er alleen met `--prices`.

`diff` heeft `inputs` als `old` en `new`, en `total_rum` als `old` en `new`. Verder `added`, `removed`, `changed_rule` (`address`, `old_rule`, `new_rule`) en `rum_delta`. `costs` is de maandschatting van de nieuwe state. `cost_delta` is het getekende verschil van de afgeronde centbedragen, in de volgorde van het prijsbestand. Beide blijven weg zonder prijsbestand. `instances`, als `--explain` aan staat, noemt eerst het oude bestand en dan het nieuwe.

Een instantie-object heeft `file`, `address`, `counts` en `rule`. `deposed` is de sleuteltekst als het object deposed is. `tainted` is `true` alleen als `status` `tainted` is.

Tekstrapporten en foutmeldingen escapen stuurtekens, en onzichtbare of tekstrichting-wijzigende opmaaktekens zoals U+200B en U+202E, in adressen, bestandspaden en prijslabels als `\uXXXX`. JSON bewaart de oorspronkelijke tekens.

## Exitcodes en gebruiksfouten

| Code | Betekenis |
| --- | --- |
| 0 | Gelukt. Een gelijke drempel hoort daarbij. |
| 1 | Drempel: opgetelde RUM boven `--max-rum` bij count, of de RUM-stijging boven `--max-increase` bij diff. Het rapport wordt toch afgedrukt. |
| 2 | Invoerfout: onleesbaar bestand, geen gewoon bestand, ongeldige JSON, een state die geen JSON-object is, stateversie niet 4, UTF-16 of UTF-32, een dubbel adres, een ongeldig prijsbestand, of kosten die overlopen. Stdout is leeg. |
| 3 | Gebruiksfout, ook een vlag die het rapport niet zou veranderen. Stdout is leeg. |

Als het rapport niet naar stdout geschreven of geflusht kan worden, is de exitcode ook 2, met `rum-meter: cannot write report to stdout`. Er kan dan al een deel van het rapport geschreven zijn. Een gesloten stdout is dezelfde fout. `--version` en `--help` die niet geschreven of geflusht kunnen worden zijn ook exit 2, niet 0, met `rum-meter: cannot write message to stdout`. Een mislukte schrijfactie geeft geen panic. Als stderr ook niet geschreven kan worden, blijft de exitcode hetzelfde en is er geen panic.

Elke rij hieronder draait tegen de binary. De melding is de hele stderr, en `\n` is een newline. Het commando en de melding zijn Engels.

| Optie | Werkt bij | Geval | Verkeerd gebruik | Exit | Melding |
| --- | --- | --- | --- | --- | --- |
| `--max-increase` | count | Op count. | `count --max-increase 0 tests/fixtures/v4_one_managed.json` | 3 | `--max-increase only works with diff\n` |
| `--max-increase` | count | Wint van een latere controle. | `count --explain --hours-per-month 1 --max-increase 0 tests/fixtures/v4_one_managed.json` | 3 | `--max-increase only works with diff\n` |
| `--max-rum` | diff | Op diff. | `diff --max-rum 0 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--max-rum only works with count\n` |
| `--max-rum` | diff | Wint van een latere controle. | `diff --explain --top 1 --hours-per-month 1 --max-rum 0 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--max-rum only works with count\n` |
| `--hours-per-month` | count | Zonder `--prices`. | `count --hours-per-month 1 tests/fixtures/v4_one_managed.json` | 3 | `--hours-per-month only works together with --prices\n` |
| `--hours-per-month` | diff | Zonder `--prices`. | `diff --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--hours-per-month only works together with --prices\n` |
| `--hours-per-month` | diff | `--json` verandert de melding niet. | `diff --json --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--hours-per-month only works together with --prices\n` |
| `--hours-per-month` | diff | `--json` maakt `--top` en `--explain` geldig, dus deze controle wint. | `diff --explain --top 1 --json --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--hours-per-month only works together with --prices\n` |
| `--top` | diff | Zonder `--json`. | `diff --top 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--top with diff only works together with --json\n` |
| `--top` | diff | Wint van `--explain`. | `diff --explain --top 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--top with diff only works together with --json\n` |
| `--explain` | diff | Zonder `--json`. | `diff --explain tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json` | 3 | `--explain with diff only works together with --json\n` |
| argumenten | count | Geen statebestand. | `count` | 3 | `error: the following required arguments were not provided:\n  <STATE>...\n\nUsage: rum-meter count <STATE>...\n\nFor more information, try '--help'.\n` |
| argumenten | diff | Het nieuwe bestand ontbreekt. | `diff only.json` | 3 | `error: the following required arguments were not provided:\n  <NEW>\n\nUsage: rum-meter diff <OLD> <NEW>\n\nFor more information, try '--help'.\n` |
| argumenten | topniveau | Onbekende vlag. | `--not-an-option` | 3 | `error: unexpected argument '--not-an-option' found\n\nUsage: rum-meter [OPTIONS] <COMMAND>\n\nFor more information, try '--help'.\n` |
| `--top` | count | Geen getal. | `count --top abc tests/fixtures/v4_one_managed.json` | 3 | `error: invalid value 'abc' for '--top <N>': invalid digit found in string\n\nFor more information, try '--help'.\n` |
| statebestand | count | Pad ontbreekt. | `count tests/fixtures/missing.json` | 2 | `rum-meter: tests/fixtures/missing.json: unreadable (not found)\n` |
| statebestand | count | Een map is geen gewoon bestand. | `count tests/fixtures` | 2 | `rum-meter: tests/fixtures: not a regular file\n` |
| statebestand | count | Leeg bestand. | `count tests/fixtures/empty.json` | 2 | `rum-meter: tests/fixtures/empty.json: invalid json at line 1 column 0\n` |
| statebestand | count | Afgekapte JSON. | `count tests/fixtures/broken.json` | 2 | `rum-meter: tests/fixtures/broken.json: invalid json at line 2 column 0\n` |
| statebestand | count | Versie 3. | `count tests/fixtures/version_3.json` | 2 | `rum-meter: tests/fixtures/version_3.json: state version is not 4\n` |
| statebestand | count | Versie is tekst, geen getal. | `count tests/fixtures/version_string.json` | 2 | `rum-meter: tests/fixtures/version_string.json: invalid json at line 1 column 33\n` |
| `--prices` | count | Een statebestand heeft geen valuta. | `count --prices tests/fixtures/v4_one_managed.json tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures/v4_one_managed.json: currency is empty\n` |
| `--prices` | count | Het prijzenpad is een map. | `count --prices tests/fixtures tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures: not a regular file\n` |
| `--prices` | count | Het prijsbestand ontbreekt. | `count --prices tests/fixtures/missing-prices.json tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures/missing-prices.json: unreadable (not found)\n` |
| `--prices` | count | Het prijsbestand is leeg. | `count --prices tests/fixtures/empty.json tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: tests/fixtures/empty.json: invalid json at line 1 column 0\n` |
| statebestand | diff | De oude state is versie 3. | `diff tests/fixtures/version_3.json tests/fixtures/v4_handwritten.json` | 2 | `rum-meter: tests/fixtures/version_3.json: state version is not 4\n` |
| statebestand | count | Een apparaat is geen gewoon bestand (Linux en macOS). | `count /dev/null` | 2 | `rum-meter: /dev/null: not a regular file\n` |
| `--prices` | count | Het prijzenpad is een apparaat (Linux en macOS). | `count --prices /dev/null tests/fixtures/v4_one_managed.json` | 2 | `rum-meter: /dev/null: not a regular file\n` |

## Privacy en veiligheid

rum-meter werkt volledig offline: geen netwerk, geen API-sleutel, geen telemetrie. De binary leest alleen de paden die je opgeeft. In de uitvoer staan adressen, modulepaden, types, regelcodes en aantallen. Attribuutwaarden, sensitive values, private data en outputs blijven achterwege, omdat die velden niet worden gedeserialiseerd. Geen foutmelding bevat bytes uit het bestand, alleen het pad, de regel en de kolom van een parsefout, en het soort fout. `tests/safety.rs` bewaakt dat met synthetische fixtures. De afhankelijkheden zijn bigdecimal, clap, serde en serde_json.
