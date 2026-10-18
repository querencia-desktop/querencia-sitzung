# Gemeinsame Hilfen der Szenarien (läuft IN der isolierten labwc-Sitzung).
set -u
. "$QS_PROFILE/env.sh"
mkdir -p "$OUT"
R=$OUT/ergebnis.txt
ok() { echo "PASS $*" | tee -a "$R"; }
nok() { echo "FAIL $*" | tee -a "$R"; }
blocked() { echo "BLOCKED $*" | tee -a "$R"; }
. "$EINGABE"
shot() { grim "$OUT/$1.png"; }
MITTE="python3 -I $(dirname "$EINGABE")/mitte.py"
# Startet das Programm mit Protokoll $OUT/$1.log, PID in $P.
start() { local log=$1; shift; "$BIN" "$@" >"$OUT/$log.log" 2>&1 & P=$!; sleep ${START_WARTEN:-2.5}; }
# Wartet auf $P, Exit-Code in $RC.
ende() { wait "$P"; RC=$?; }
# Sicherheitsnetz: LABWC_PID muss der isolierte labwc sein.
isoliert_pruefen() {
    tr '\0' '\n' <"/proc/$LABWC_PID/environ" | grep -q "^XDG_RUNTIME_DIR=.*/qed-runtime-" ||
        { nok "Sicherheitsnetz: LABWC_PID $LABWC_PID ist nicht der isolierte labwc"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
    [ "${QS_TROCKENLAUF:-}" = 1 ] || { nok "Sicherheitsnetz: QS_TROCKENLAUF fehlt"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
}
isoliert_pruefen
