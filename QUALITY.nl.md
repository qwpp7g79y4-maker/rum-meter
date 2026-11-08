# Kwaliteitsrapport

Deze pagina laat zien hoe elke release van rum-meter is gecontroleerd voordat hij online kwam, wat we vonden en wat we hebben opgelost. Het beschrijft onze controles. Het is geen garantie dat de software foutloos of veilig is.

## Pre-release 1.0.0 (tak prep/w24-fout)

| | |
|---|---|
| Versie | 1.0.0 (niet gepubliceerd vanaf deze tak) |
| Gemeten commit | `decfa7b` (tak prep/w24-fout) |
| Tests | 286 passed, 0 failed, 0 ignored (`cargo test --no-fail-fast`, debugmodus) |
| Lints | `cargo fmt --check` en `cargo clippy --all-targets -- -D warnings` schoon |

## Regeldekking (pre-release 1.0.0)

| | |
|---|---|
| Datum | 2026-11-08 |
| Gemeten commit | `decfa7b` (tak prep/w24-fout) |
| Commando | `cargo llvm-cov --no-fail-fast -- --skip symlink_becomes_fifo_between_name_and_open` |
| Regeldekking | 96.78% (2920 regels, 94 gemist), zoals het commando het afdrukt |
| Doel van 90% of meer | Gehaald. Er zijn geen tests voor de dekking toegevoegd. |

Eén test wordt alleen in de dekkingsrun overgeslagen: `symlink_becomes_fifo_between_name_and_open_exits_2` in `tests/regular_file.rs`. Hij traceert het kindproces met ptrace en staat 2 seconden toe. In een gewone `cargo test` duurt hij ongeveer 1.7 seconden en slaagt hij. Het binary dat llvm-cov bouwt is geïnstrumenteerd en schrijft bij het afsluiten een `.profraw`-bestand, waardoor hij over de limiet van 2 seconden gaat en daar in 2 van 3 runs faalt. De test is ongewijzigd en draait in de normale `cargo test`. De overgeslagen test dekt een race bij het openen van een pad. Zijn regels staan in `src/regular.rs`, dat zonder hem op 93.48% staat.

## Gemeten tijd op een invoer van 100 MiB (pre-release 1.0.0)

| | |
|---|---|
| Datum | 2026-11-08 |
| Gemeten commit | `decfa7b` (tak prep/w24-fout) |
| Bouw | `cargo build --release` |
| Invoer | Eén synthetische Terraform-state, 104857600 bytes (precies 100 MiB), formaat versie 4 zoals in `examples/`: 492,809 managed `aws_instance`-resources, elk één instantie. De resource-entries zijn bijna identiek: alleen de naam en het instance-id veranderen. Het bestand wordt in `/tmp` gegenereerd en niet gecommit. |
| Commando | `/usr/bin/time -f "%e s" target/release/rum-meter count big.tfstate` (exit 0, 492809 RUM) |
| Gemeten | 0.85 s, 0.86 s, 0.86 s (drie runs, verstreken wall-clock-seconden) |

Dit is één machine en één invoervorm. Echte states verschillen, dus het is geen belofte voor andere invoer. Er wordt geen doorvoergetal gegeven.

## v0.1.2

| | |
|---|---|
| Versie | 0.1.2 ([release](https://github.com/qwpp7g79y4-maker/rum-meter/releases/tag/v0.1.2), [crates.io](https://crates.io/crates/rum-meter/0.1.2)) |
| Gecontroleerde commit | [`adc53d2`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/adc53d2) |
| Datum | 2026-10-25 |
| Tests | 232 automatische tests, gedraaid in debug- en releasemodus |

### Hoe we controleerden

- Dezelfde controles als bij de vorige versie (hieronder), op elk platform.
- Geautomatiseerde tests in CI op Linux, Windows, macOS op Apple Silicon en Intel, en Rust 1.85, plus `cargo audit` (geen bekende kwetsbaarheden) en `cargo deny check licenses` (ok).
- `cargo clippy --all-targets` en `cargo fmt --check` schoon, op debug- en release-testruns.
- Een `cargo-fuzz`-harness (`fuzz/`) met één doel per onbetrouwbare-invoerstap (state parsen, prijstier parsen, kostenberekening, `diff`, weergave); CI bouwt deze doelen alleen bij elke push en pull request (`.github/workflows/ci.yml:76-88`), voert ze niet uit, en er is geen periodieke (nachtelijke) trigger.

### Wat we vonden en oplosten

| Probleem | Oplossing |
|---|---|
| `diff` matchte objecten op het afgedrukte adres, waardoor een resource met de naam `a[0]` en resource `a` op index 0 als geen wijziging leken | Matcht nu op module, mode, type, naam, indexsleutel en deposed-sleutel |
| Een prijsbestand met twee tiers met dezelfde naam werd geaccepteerd | Exit 2, `duplicate tier name`, geen kosten afgedrukt |
| Een herhaalde sleutel binnen één prijstier werd gemeld als `tier is not an object` | Het is `duplicate key in a tier` |
| Tekstuitvoer escapete U+2028/U+2029 niet, waardoor een adres, pad, module, type of prijslabel met zo'n teken een extra regel in een rapport kon voorwenden | Ge-escaped op dezelfde manier als andere onzichtbare/bidirectionele tekens |
| `docs/reference.md` en `docs/reference.nl.md` noemden een verouderde `--version`-tekst (`rum-meter 0.1.0`) | Volgt nu de crateversie; een test pint `--version` daarop |
| Het Windows-`extern`-blok was niet `unsafe`, waardoor de crate niet bouwde op edition 2024 | Gemarkeerd als `unsafe` |
| Een `currency` in een prijsbestand langer dan 16 tekens, of buiten zichtbare ASCII, werd op elke tier-regel gekopieerd voordat iets dat afwees | Geweigerd bij het laden, voor er een kopie bestaat |
| Een state kon een gedeeld resource-label (module, mode, type of naam) over te veel instanties herhalen voordat classificatie dat opving | Direct geweigerd, voor classificatie |
| De logische grootte van een sparse bestand, niet de echte inhoud, bepaalde hoeveel van een state-, prijs- of stdin-lezing werd toegewezen en gelezen voordat de JSON-parser kon falen | Lezen stopt zodra het lopende totaal de cap van 128 MiB zou overschrijden |
| Het gedeelde labelbudget van 32 MiB van `count` werd per bestand gecontroleerd, zodat meerdere bestanden elk net onder het budget, of één herhaald pad, erdoorheen glipten | Budget is een lopend totaal over de hele aanroep; een herhaald pad wordt geweigerd voor enig bestand opent |
| `mode` werd door `classify` op elke instantie gekopieerd voordat het meetelde in het gedeelde labelbudget | Telt nu eerst mee in het budget |

Alle fixes staan in de [changelog](https://github.com/qwpp7g79y4-maker/rum-meter/blob/main/CHANGELOG.md).

## v0.1.1

| | |
|---|---|
| Versie | 0.1.1 ([release](https://github.com/qwpp7g79y4-maker/rum-meter/releases/tag/v0.1.1), [crates.io](https://crates.io/crates/rum-meter/0.1.1)) |
| Gecontroleerde commit | [`d6f3fe2`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/d6f3fe2) |
| Datum | 2026-10-15 |
| Tests | 176 automatische tests, gedraaid in debug- en releasemodus |

### Hoe we controleerden

- Dezelfde controles als bij de vorige versie (hieronder), op elk platform.
- Een tweede externe geautomatiseerde codereview. Elke bevinding die we konden reproduceren is in deze versie opgelost, elk met een regressietest met precies die invoer.
- Geautomatiseerde tests in CI: [run 38654547261](https://github.com/qwpp7g79y4-maker/rum-meter/actions/runs/38654547261) op Linux, Windows, macOS op Apple Silicon en Intel, en Rust 1.85, plus `cargo audit` (geen bekende kwetsbaarheden).
- Een verse onafhankelijke eindreview op deze commit tegen een vaste edge-case-checklist (elk punt met een concrete kwade invoer). Die eindigde op DONE; geen blokkerende bevindingen.
- Nog niet uitgebracht: een latere verse eindreview van commit [`195b64c`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/195b64c) vond dat `diff` een echte verwijdering en toevoeging als geen wijziging kon tonen wanneer twee objecten hetzelfde adres tonen (een resource met de naam `a[0]` en resource `a` op index 0). Dat is opgelost in [`37e050a`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/37e050a), met regressietests. Een verse onafhankelijke eindreview van commit [`37e050a`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/37e050a), tegen dezelfde vaste checklist met elk punt geprobeerd met een concrete kwade invoer, eindigde op DONE, zonder blokkerende bevindingen.

### Wat we vonden en oplosten

| Probleem | Oplossing |
|---|---|
| Een prijs kon via binaire drijvende komma worden afgerond (één cent verschil) | Prijzen worden als exacte decimalen gelezen |
| Een statebestand met een JSON-array als wortel werd geaccepteerd | Het wordt geweigerd met exit 2 |
| Hetzelfde adres in meerdere invoerbestanden werd dubbel geteld | Het wordt geweigerd |
| Een pad dat door een FIFO werd vervangen kon de tool laten wachten; een volle schijf op stdout of stderr gaf een panic | Het geopende bestand wordt gecontroleerd; schrijffouten zijn nette fouten en geven nooit exit 0 |
| `--top` gaf twee niet te onderscheiden `other`-rijen als een echte module of type `other` heette | De rest is `(other)` (daarna `(other 2)`, enzovoort) zodat elke naam uniek blijft |
| Een herhaalde sleutel in een prijstier werd gemeld als `tier is not an object` | Het is `duplicate key in a tier` (exit 2) |

Alle fixes staan in de [changelog](https://github.com/qwpp7g79y4-maker/rum-meter/blob/main/CHANGELOG.md).

## v0.1.0

| | |
|---|---|
| Versie | 0.1.0 ([release](https://github.com/qwpp7g79y4-maker/rum-meter/releases/tag/v0.1.0), [crates.io](https://crates.io/crates/rum-meter/0.1.0)) |
| Gecontroleerde commit | [`32f8402`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/32f8402) |
| Datum | 2026-10-11 |
| Tests | 158 automatische tests |

### Wat we controleerden

**Codereview.** De code is gelezen aan de hand van negen vaste vragen:
1. Geeft het het juiste antwoord?
2. Wat gebeurt er bij foute of ontbrekende invoer?
3. Wat gebeurt er als het halverwege misgaat, bijvoorbeeld als de uitvoer niet geschreven kan worden?
4. Welke aannames doet het, en staan die beschreven?
5. Is het veilig als dingen tegelijk gebeuren?
6. Kan onbetrouwbare invoer in de uitvoer lekken of schade doen, bijvoorbeeld via verborgen tekens?
7. Blijft het snel en zuinig bij grote invoer?
8. Is elke bewering gedekt door een test?
9. Is de code goed te lezen en aan te passen?

**Invoer van elk platform.** UTF-8 met en zonder byte order mark, UTF-16 en UTF-32, Windows-regeleinden (CRLF) en bestandspaden die geen geldige UTF-8 zijn.

**Tests op elk platform.** Elke wijziging draait alle tests op Linux, Windows, macOS op Apple Silicon en macOS op Intel, plus de oudste ondersteunde Rust-versie (1.85).

**Release.** De release wordt pas gebouwd als de tests op de getagde commit opnieuw slagen. Elke download wordt voor publicatie uitgepakt en één keer gestart, en krijgt een SHA-256-checksum.

**Afhankelijkheden.** `cargo audit` op 2026-10-12 tegen de RustSec-advisorydatabase (1296 meldingen): geen bekende kwetsbaarheden of waarschuwingen in de 35 crate-afhankelijkheden. Dit draait nu bij elke wijziging in CI.

### Hoe we controleerden

- Onafhankelijke codereview, door reviewers die de code niet schreven.
- Een externe automatische codereview.
- Automatische tests in CI. Runs: [release v0.1.0](https://github.com/qwpp7g79y4-maker/rum-meter/actions/runs/38174144815), [laatste main](https://github.com/qwpp7g79y4-maker/rum-meter/actions/runs/38203196595).

We noemen de reviewdiensten hier niet. Het gaat om de controles en hun uitkomst, en alles hieronder is na te gaan in de code en de changelog.

### Wat we vonden en oplosten vóór de release

| Probleem | Oplossing |
|---|---|
| Kosten konden verkeerd worden afgerond door kommagetallen | Decimaal rekenen, half van nul af afgerond op hele centen |
| Een overflow was mogelijk bij 0 uur | 0 uur geeft altijd 0.00 |
| Stuurtekens in adressen, labels en paden konden het rapport verstoren | Ze worden ge-escaped in tekstuitvoer |
| Onzichtbare en bidirectionele tekens konden een regel anders laten lezen dan hij is | Ze worden ge-escaped in tekstuitvoer |
| Een mislukte schrijfactie naar stdout (bijvoorbeeld bij `head`) werd niet gemeld | Exitcode 2 met een duidelijke melding |
| UTF-16- en UTF-32-bestanden werden niet duidelijk afgehandeld | Ze worden geweigerd met een duidelijke melding |
| Een UTF-8 byte order mark brak het inlezen | Die wordt overgeslagen |

Alle fixes zitten in de eerste openbare versie, zie de [changelog](https://github.com/qwpp7g79y4-maker/rum-meter/blob/main/CHANGELOG.md#010---2026-10-11).

### Niet gedekt

- Alleen Terraform-state formaat versie 4 wordt ondersteund.
- De kostenschatting gebruikt prijzen die je zelf opgeeft. Het is geen officiële rekening.

## Een probleem melden

Open een [issue](https://github.com/qwpp7g79y4-maker/rum-meter/issues), of volg bij beveiligingsproblemen [SECURITY.md](SECURITY.md). Contact: pep_ai@icloud.com.
