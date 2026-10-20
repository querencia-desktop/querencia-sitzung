#!/bin/bash
# G2-AI (QD-062): echtes Abmelden im ISOLIERTEN labwc beendet zuerst die
# Sitzungsprogramme (SIGTERM, Frist, SIGKILL), dann labwc --exit.
# Programme anderer Sitzungen (andere LABWC_PID) und Benutzerprogramme
# bleiben. Sicherheitsnetz wie szenario-abmelden.sh (gemeinsam.sh).
. "$(dirname "$0")/gemeinsam.sh"
PL=$QS_PROFILE/platzhalter; mkdir -p "$PL/a" "$PL/b" "$PL/c"
gcc -O1 -o "$PL/platzhalter" "$(dirname "$0")/platzhalter.c" || { nok "A02 Platzhalter nicht übersetzt"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
cp "$PL/platzhalter" "$PL/a/querencia-polkit-agent"
cp "$PL/platzhalter" "$PL/a/querencia-lautstaerke"
cp "$PL/platzhalter" "$PL/a/querencia-netzwerk"
cp "$PL/platzhalter" "$PL/b/querencia-panel"
cp "$PL/platzhalter" "$PL/c/strata"
# Sitzungsprogramme dieser Sitzung
"$PL/a/querencia-polkit-agent" & s1=$!
"$PL/a/querencia-lautstaerke" & s2=$!
"$PL/a/querencia-netzwerk" taub & s3=$!
"$BIN" --leerlauf-sekunden 600 >"$OUT/a02-leerlauf.log" 2>&1 & s4=$!
"$PL/b/querencia-panel" & s5=$!
# Köder: gleiches Programm, andere Sitzung; Benutzerprogramm dieser Sitzung
env LABWC_PID=1 "$PL/b/querencia-panel" & k1=$!
"$PL/c/strata" & k2=$!
sleep 1.5
SITZ="$s1 $s2 $s3 $s4 $s5"; KOED="$k1 $k2"
echo "sitzung $SITZ" >"$OUT/a02-pids.txt"; echo "koeder $KOED" >>"$OUT/a02-pids.txt"
ps -o pid,args -p "${SITZ// /,},${KOED// /,}" >"$OUT/a02-vorher.txt"
# Beobachter außerhalb des Prozessbaums der Sitzung (überlebt labwc kurz)
env XDG_RUNTIME_DIR=/nonexistent setsid bash -c '
    for i in $(seq 1 80); do
        echo "t=$i $(for p in '"$SITZ"'; do [ -d /proc/$p ] && grep -q "^State:.*[^Z] (" /proc/$p/status 2>/dev/null && echo -n "S$p "; done)$(for p in '"$KOED"'; do [ -d /proc/$p ] && echo -n "K$p "; done)"
        sleep 0.1
    done' >"$OUT/a02-beobachter.txt" 2>&1 &
echo "A02 erwartet: labwc $LABWC_PID endet nach dem Beenden von $SITZ; $KOED bleiben" >"$OUT/a02-erwartung.txt"
env -u QS_TROCKENLAUF "$BIN" --abmelden --countdown 1 >"$OUT/a02.log" 2>&1
echo "exit $?" >>"$OUT/a02.log"
sleep 3
nok "A02 isolierter labwc läuft nach --abmelden noch ($(tr '\n' ';' <"$OUT/a02.log"))"
touch "$QS_PROFILE/logs/FERTIG"
