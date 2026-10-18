#!/bin/bash
# Leerlaufdienst: sperrt nach 4 s ohne Eingabe, erneut nach Aktivität,
# startet keine zweite Sperre, solange eine läuft.
. "$(dirname "$0")/gemeinsam.sh"
export QS_TESTPASSWORT=querencia-test
"$BIN" --leerlauf-sekunden 4 >"$OUT/l01.log" 2>&1 & L=$!
sleep 1; q "move 100 100" "wait 100"; sleep 6.5
grep -q 'Leerlauf erreicht' "$OUT/l01.log" && grep -q 'sperren --sofort gestartet' "$OUT/l01.log" && ok "L01 Leerlauf 4 s ohne Eingabe: Sperre gestartet ($(grep -o 'Nr. [0-9]*' "$OUT/l01.log" | tail -1))" || nok "L01 Leerlauf: $(tr '\n' ';' <"$OUT/l01.log")"
shot l01-leerlauf-gesperrt
"$BIN" --sperren --sofort >"$OUT/l02.log" 2>&1; rc=$?
[ $rc = 2 ] && ok "L02 Sitzung ist wirklich gesperrt (zweite Sperre abgelehnt)" || nok "L02 (Exit $rc)"
# Entsperren, dann sofort ein Fenster zeigen und schließen (vor dem nächsten Leerlauf)
q "type querencia-test" "key Return" "wait 1200" "move 200 200"
START_WARTEN=1.2 start l03 --dialog; shot l03-nach-entsperren
kill -0 $P 2>/dev/null && ok "L03 nach dem Entsperren öffnet ein Fenster" || nok "L03"
q "key Escape" "wait 300"; ende
sleep 7
[ "$(grep -c 'Leerlauf erreicht' "$OUT/l01.log")" -ge 2 ] && grep -q 'Nr. 2' "$OUT/l01.log" && ok "L04 nach Aktivität sperrt der Leerlauf erneut (Nr. 2)" || nok "L04: $(tr '\n' ';' <"$OUT/l01.log")"
# Während der Sperre läuft der Leerlauf weiter ab: kein zweiter Sperrprozess
sleep 5
grep -q 'läuft bereits' "$OUT/l01.log" && [ "$(grep -c 'sofort gestartet' "$OUT/l01.log")" = 2 ] && ok "L05 erneuter Leerlauf während der Sperre startet keine zweite Sperre" || nok "L05: $(tr '\n' ';' <"$OUT/l01.log")"
q "type querencia-test" "key Return" "wait 1500"
kill -TERM $L; wait $L 2>/dev/null
touch "$QS_PROFILE/logs/FERTIG"
