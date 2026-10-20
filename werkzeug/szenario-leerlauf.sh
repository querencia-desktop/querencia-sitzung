#!/bin/bash
# Leerlaufdienst: sperrt nach 8 s ohne Eingabe, erneut nach Aktivität,
# startet keine zweite Sperre, solange eine läuft, und erntet jeden beendeten
# Sperrprozess sofort (kein Zombie, QD-050).
. "$(dirname "$0")/gemeinsam.sh"
# Zombie-Kinder des Leerlaufdienstes (Zustand Z), nur Prozesse des isolierten Laufs
zombies() { ps -o pid=,stat= --ppid "$L" 2>/dev/null | awk '$2 ~ /^Z/ {print $1}' | tr '\n' ' '; }
export QS_TESTPASSWORT=querencia-test
"$BIN" --leerlauf-sekunden 8 >"$OUT/l01.log" 2>&1 & L=$!
sleep 1; q "move 100 100" "wait 100"; sleep 10.5
grep -q 'Leerlauf erreicht' "$OUT/l01.log" && grep -q 'sperren --sofort gestartet' "$OUT/l01.log" && ok "L01 Leerlauf 8 s ohne Eingabe: Sperre gestartet ($(grep -o 'Nr. [0-9]*' "$OUT/l01.log" | tail -1))" || nok "L01 Leerlauf: $(tr '\n' ';' <"$OUT/l01.log")"
shot l01-leerlauf-gesperrt
"$BIN" --sperren --sofort >"$OUT/l02.log" 2>&1; rc=$?
[ $rc = 2 ] && ok "L02 Sitzung ist wirklich gesperrt (zweite Sperre abgelehnt)" || nok "L02 (Exit $rc)"
# Entsperren, dann sofort ein Fenster zeigen und schließen (vor dem nächsten Leerlauf)
# Entsperren; der Dialog braucht ~2 s bis zur Anzeige, dann Klick und Esc,
# alles innerhalb der 8 s seit der letzten Eingabe (dem Passwort).
q "type querencia-test" "key Return" "wait 800"
start l03 --dialog; shot l03-nach-entsperren
$MITTE "$OUT/l03-nach-entsperren.png" >/dev/null && kill -0 $P 2>/dev/null && ok "L03 nach dem Entsperren öffnet ein Fenster (mittig)" || nok "L03"
q "move 640 360" "click" "wait 200" "key Escape" "wait 500"; ende
# L06 der beendete Sperrprozess Nr. 1 ist schon geerntet, ohne neues Leerlauf-Ereignis
[ -z "$(zombies)" ] && grep -q 'geerntet' "$OUT/l01.log" && ok "L06 nach dem Entsperren kein Zombie-Kind des Leerlaufdienstes (QD-050)" || nok "L06 Zombie-Kinder: $(zombies) / $(grep 'beendet' "$OUT/l01.log" | tr '\n' ';')"
sleep 10.5
[ "$(grep -c 'Leerlauf erreicht' "$OUT/l01.log")" -ge 2 ] && grep -q 'Nr. 2' "$OUT/l01.log" && ok "L04 nach Aktivität sperrt der Leerlauf erneut (Nr. 2)" || nok "L04: $(tr '\n' ';' <"$OUT/l01.log")"
# Während der Sperre läuft der Leerlauf weiter ab: kein zweiter Sperrprozess
q "move 120 120" "wait 100"; sleep 10.5
grep -q 'läuft bereits' "$OUT/l01.log" && [ "$(grep -c 'sofort gestartet' "$OUT/l01.log")" = 2 ] && ok "L05 erneuter Leerlauf während der Sperre startet keine zweite Sperre" || nok "L05: $(tr '\n' ';' <"$OUT/l01.log")"
q "type querencia-test" "key Return" "wait 1500"
[ -z "$(zombies)" ] && ok "L07 nach dem zweiten Entsperren kein Zombie-Kind" || nok "L07 Zombie-Kinder: $(zombies)"
kill -TERM $L; wait $L 2>/dev/null
touch "$QS_PROFILE/logs/FERTIG"
