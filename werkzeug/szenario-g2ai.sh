#!/bin/bash
# G2-AI (QD-062): Vorgabe ohne Countdown (Entscheidung des Besitzers 20.10.),
# Countdown nur auf Wunsch (--countdown, sitzung.ini Countdown=an), dann nur
# mit Fokus und Abbruch nach 60 s ohne Fokus. Alles im Trockenlauf.
. "$(dirname "$0")/gemeinsam.sh"
INI=$XDG_CONFIG_HOME/querencia/sitzung.ini
mkdir -p "$(dirname "$INI")"; rm -f "$INI"
fenster_weg() { kill -0 "$P" 2>/dev/null && echo offen || echo zu; }

# C01 mit Fokus: Countdown 4 s läuft ab
START_WARTEN=1 start c01 --abmelden --countdown 4; shot c01-countdown; sleep 4.5; ende
[ $RC = 0 ] && grep -q 'TROCKENLAUF: .*labwc --exit' "$OUT/c01.log" && ok "C01 mit Fokus: Countdown 4 s löst aus (Trockenlauf)" || nok "C01 (Exit $RC): $(tr '\n' ';' <"$OUT/c01.log")"

# C02 ohne Fokus angehalten, mit Fokus weiter
START_WARTEN=1.5 start c02 --abmelden --countdown 4
foot -W 24x3 -T fokusdieb sleep 600 >"$OUT/c02-foot.log" 2>&1 & F=$!
sleep 8; shot c02-angehalten
zustand=$(fenster_weg)
grep -q 'Countdown angehalten' "$OUT/c02.log" && [ "$zustand" = offen ] && ! grep -q TROCKENLAUF "$OUT/c02.log" &&
    ok "C02a anderes Fenster aktiv: Countdown angehalten, nach 8 s nichts ausgelöst" || nok "C02a ($zustand): $(tr '\n' ';' <"$OUT/c02.log")"
kill $F; wait $F 2>/dev/null
sleep 5; ende
grep -q 'Countdown läuft weiter' "$OUT/c02.log" && grep -q 'TROCKENLAUF: .*labwc --exit' "$OUT/c02.log" &&
    ok "C02b Fokus zurück: Countdown läuft weiter und löst aus" || nok "C02b (Exit $RC): $(tr '\n' ';' <"$OUT/c02.log")"

# C03 60 s ohne Fokus: Abbruch, keine Aktion
START_WARTEN=1.5 start c03 --herunterfahren --countdown 30
foot -W 24x3 -T fokusdieb sleep 600 >"$OUT/c03-foot.log" 2>&1 & F=$!
sleep 63; ende; kill $F; wait $F 2>/dev/null
[ $RC = 0 ] && grep -q '60 s ohne Fokus, Dialog abgebrochen' "$OUT/c03.log" && grep -q '^querencia-sitzung: abgebrochen' "$OUT/c03.log" && ! grep -q 'logind\|TROCKENLAUF' "$OUT/c03.log" &&
    ok "C03 60 s ohne Fokus: Dialog bricht ab, keine Aktion (Exit 0)" || nok "C03 (Exit $RC): $(tr '\n' ';' <"$OUT/c03.log")"

# C04 Vorgabe: kein Countdown, die Rückfrage wartet
rm -f "$INI"
START_WARTEN=1 start c04 --abmelden; shot c04-ohne-countdown; sleep 6
zustand=$(fenster_weg); q "key Escape" "wait 800"; ende
[ "$zustand" = offen ] && ! grep -q 'TROCKENLAUF\|Countdown' "$OUT/c04.log" && grep -q 'abgebrochen' "$OUT/c04.log" &&
    ok "C04 Vorgabe ohne Countdown: Rückfrage wartet 6 s ohne Aktion; Esc bricht ab" || nok "C04 ($zustand): $(tr '\n' ';' <"$OUT/c04.log")"

# C05 Enter auf dem Standardknopf (Abbrechen) löst nichts aus
START_WARTEN=1 start c05 --herunterfahren; q "key Return" "wait 800"; ende
[ $RC = 0 ] && grep -q '^querencia-sitzung: abgebrochen' "$OUT/c05.log" && ! grep -q 'logind\|TROCKENLAUF' "$OUT/c05.log" &&
    ok "C05 Enter auf Standardknopf: abgebrochen, keine Aktion" || nok "C05 (Exit $RC): $(tr '\n' ';' <"$OUT/c05.log")"

# C07 sitzung.ini Countdown=an: 60-s-Countdown auf Wunsch; Befehlszeile geht vor
printf '[Dialog]\nCountdown=an\n' >"$INI"
START_WARTEN=1.5 start c07 --abmelden; shot c07-countdown-an; q "key Escape" "wait 800"; ende
START_WARTEN=1 start c07b --abmelden --countdown 2; sleep 2.5; ende
grep -q 'Countdown an' "$OUT/c07.log" && grep -q 'TROCKENLAUF: .*labwc --exit' "$OUT/c07b.log" && ! grep -q 'Countdown an' "$OUT/c07b.log" &&
    ok "C07 sitzung.ini Countdown=an schaltet ein; --countdown 2 geht vor" || nok "C07: $(tr '\n' ';' <"$OUT/c07.log") / $(tr '\n' ';' <"$OUT/c07b.log")"
rm -f "$INI"

# C08 --kein-countdown gibt es nicht mehr
"$BIN" --abmelden --kein-countdown >"$OUT/c08.log" 2>&1; rc=$?
[ $rc = 2 ] && ok "C08 --kein-countdown: Exit 2 (Option entfällt)" || nok "C08 Exit $rc"

# C06 Gesamtdialog mit Countdown ohne Fokus ebenfalls angehalten
rm -f "$INI"
START_WARTEN=1.5 start c06 --dialog --countdown 3
foot -W 24x3 -T fokusdieb sleep 600 >"$OUT/c06-foot.log" 2>&1 & F=$!
sleep 6; zustand=$(fenster_weg); kill $F; wait $F 2>/dev/null; sleep 1
q "key Escape" "wait 800"; ende
[ "$zustand" = offen ] && ! grep -q TROCKENLAUF "$OUT/c06.log" && ok "C06 --dialog --countdown 3 ohne Fokus: nach 6 s nichts ausgelöst" || nok "C06 ($zustand): $(tr '\n' ';' <"$OUT/c06.log")"
touch "$QS_PROFILE/logs/FERTIG"
