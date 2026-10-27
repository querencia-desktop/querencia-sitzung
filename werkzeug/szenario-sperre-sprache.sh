#!/bin/bash
# Sperrbildschirm in der Sprache von QS_SPRACHE (Sitzung 0.1.10): Hinweis,
# Platzhalter, Datum, leere Eingabe, falsches Passwort, Punkte, Entsperren.
# Nur Testpasswort (kein PAM), ein Ausgang in QS_GROESSE (Vorgabe 1024x768).
. "$(dirname "$0")/gemeinsam.sh"
export QS_TESTPASSWORT=querencia-test
# neutraler Name auf den Fotos (keine Klarnamen in Belegen)
export USER=probe
SPR=${LANGUAGE:-de}
"$QA" "${QS_GROESSE:-1024x768}" >"$OUT/l-$SPR-qausgabe.log" 2>&1; sleep 1.5
start "l-$SPR" --sperren --sofort
grep -q 'Sitzung gesperrt' "$OUT/l-$SPR.log" && ok "L01 [$SPR] gesperrt ($(grep -o 'Sperrfläche auf [^ ]* [0-9×]*' "$OUT/l-$SPR.log" | head -1))" || nok "L01 [$SPR] Sperre"
grep -q "/$SPR/LC_MESSAGES/querencia-sitzung.mo" "/proc/$P/maps" 2>/dev/null && ok "L02 [$SPR] Katalog geladen" ||
    { [ "$SPR" = en ] && ok "L02 [en] englisch (msgid)" || nok "L02 [$SPR] Katalog nicht geladen"; }
shot "l01-$SPR-start"
q "key Return" "wait 600"; shot "l02-$SPR-leer"
q "type falsch" "wait 300"; shot "l03-$SPR-punkte"
q "key Return" "wait 800"; shot "l04-$SPR-falsch"
kill -0 $P 2>/dev/null && ok "L03 [$SPR] falsches Passwort: Sperre bleibt" || nok "L03 [$SPR] Sperre beendet"
q "type querencia-test" "key Return" "wait 1500"; ende
[ $RC = 0 ] && ok "L04 [$SPR] Testpasswort entsperrt (Exit 0)" || nok "L04 [$SPR] Entsperren (Exit $RC)"
touch "$QS_PROFILE/logs/FERTIG"
