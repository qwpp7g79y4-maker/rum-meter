# Kwaliteitsrapport

Deze pagina laat zien hoe elke release van rum-meter is gecontroleerd voordat hij online kwam, wat we vonden en wat we hebben opgelost. Het beschrijft onze controles. Het is geen garantie dat de software foutloos of veilig is.

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
- Nog niet uitgebracht: een latere verse eindreview van commit [`195b64c`](https://github.com/qwpp7g79y4-maker/rum-meter/commit/195b64c) vond dat `diff` een echte verwijdering en toevoeging als geen wijziging kon tonen wanneer twee objecten hetzelfde adres tonen (een resource met de naam `a[0]` en resource `a` op index 0). Dat is daarna opgelost, met regressietests; de gefixte code heeft nog geen eindreview gehad die op DONE eindigde.

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
