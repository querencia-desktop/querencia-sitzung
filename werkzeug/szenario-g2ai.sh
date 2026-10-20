#!/bin/bash
# G2-AI (QD-062): Countdown nur mit Fokus, Abbruch nach 60 s ohne Fokus,
# Abschalten über sitzung.ini. Alles im Trockenlauf.
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

# C04 sitzung.ini Countdown=aus: kein Countdown
printf '[Dialog]\nCountdown=aus\n' >"$INI"
START_WARTEN=1 start c04 --abmelden; shot c04-ohne-countdown; sleep 6
zustand=$(fenster_weg); q "key Escape" "wait 800"; ende
grep -q 'Countdown aus' "$OUT/c04.log" && [ "$zustand" = offen ] && ! grep -q TROCKENLAUF "$OUT/c04.log" &&
    ok "C04 sitzung.ini Countdown=aus: kein Countdown, Dialog wartet; Esc bricht ab" || nok "C04 ($zustand): $(tr '\n' ';' <"$OUT/c04.log")"

# C05 Befehlszeile geht vor
START_WARTEN=1 start c05 --abmelden --countdown 2; sleep 2.5; ende
grep -q 'TROCKENLAUF: .*labwc --exit' "$OUT/c05.log" && ! grep -q 'Countdown aus' "$OUT/c05.log" &&
    ok "C05 --countdown 2 geht sitzung.ini vor" || nok "C05: $(tr '\n' ';' <"$OUT/c05.log")"

# C06 Gesamtdialog mit Countdown ohne Fokus ebenfalls angehalten
rm -f "$INI"
START_WARTEN=1.5 start c06 --dialog --countdown 3
foot -W 24x3 -T fokusdieb sleep 600 >"$OUT/c06-foot.log" 2>&1 & F=$!
sleep 6; zustand=$(fenster_weg); kill $F; wait $F 2>/dev/null; sleep 1
q "key Escape" "wait 800"; ende
[ "$zustand" = offen ] && ! grep -q TROCKENLAUF "$OUT/c06.log" && ok "C06 --dialog --countdown 3 ohne Fokus: nach 6 s nichts ausgelöst" || nok "C06 ($zustand): $(tr '\n' ';' <"$OUT/c06.log")"
touch "$QS_PROFILE/logs/FERTIG"
