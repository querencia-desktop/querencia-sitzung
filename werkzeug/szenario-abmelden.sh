#!/bin/bash
# Echtes Abmelden im ISOLIERTEN labwc: ohne Trockenlauf ruft --abmelden
# „<labwc> --exit“ auf, der isolierte labwc endet. Das Sicherheitsnetz in
# gemeinsam.sh stellt sicher, dass LABWC_PID der isolierte labwc ist.
. "$(dirname "$0")/gemeinsam.sh"
echo "A01 erwartet: labwc $LABWC_PID ($(readlink /proc/$LABWC_PID/exe)) endet durch --exit" >"$OUT/a01-erwartung.txt"
env -u QS_TROCKENLAUF "$BIN" --abmelden --countdown 2 >"$OUT/a01.log" 2>&1
echo "exit $?" >>"$OUT/a01.log"
sleep 3
# Hierher kommt es nur, wenn labwc NICHT beendet wurde.
nok "A01 --abmelden ohne Trockenlauf: isolierter labwc läuft noch ($(cat "$OUT/a01.log" | tr '\n' ';'))"
touch "$QS_PROFILE/logs/FERTIG"
