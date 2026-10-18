#!/bin/bash
# Kalibrierung: Dialog zeigen und Bild machen; Sperre zeigen und Bild machen.
set -u
. "$QS_PROFILE/env.sh"
mkdir -p "$OUT"
. "$EINGABE"
export QS_TESTPASSWORT=querencia-test
"$BIN" --dialog >"$OUT/probe-dialog.log" 2>&1 &
P=$!
sleep 2.5
q "move 640 600" "wait 300"
grim "$OUT/probe-01-dialog.png"
q "key Escape" "wait 800"
wait $P; echo "dialog exit $?"
"$BIN" --abmelden >"$OUT/probe-abmelden.log" 2>&1 &
P=$!
sleep 2.5
grim "$OUT/probe-02-abmelden.png"
q "key Escape" "wait 800"
wait $P; echo "abmelden exit $?"
"$BIN" --sperren --sofort >"$OUT/probe-sperre.log" 2>&1 &
P=$!
sleep 2.5
grim "$OUT/probe-03-sperre.png"
q "type querencia-test" "wait 300"
grim "$OUT/probe-04-sperre-eingabe.png"
q "key Return" "wait 1500"
wait $P; echo "sperre exit $?"
cat "$OUT"/probe-*.log
touch "$QS_PROFILE/logs/FERTIG"
