#!/bin/bash
# Isolierte GUI-Prüfung von Querencia Sitzung. Nur Wegwerfprofil, eigener
# headless labwc, eigener D-Bus; die laufende Sitzung bleibt unberührt.
#   bash werkzeug/gui-pruefung.sh [szenario.sh] [ausgaenge] [zeitlimit]
# Standard: alle Szenarien nacheinander.
# QS_OHNE_SYSTEM=1 (Vorgabe): Fälle, die das echte System fragen (logind auf
# dem Systembus, echtes PAM), werden als BLOCKED ausgelassen, das
# polkit-Szenario (registriert den Agenten bei der echten logind-Sitzung)
# ganz; QS_OHNE_SYSTEM=0 nur mit ausdrücklicher Freigabe.
# QS_SPRACHE=de|en setzt LANGUAGE (Vorgabe de wie die Sitzung).
set -euo pipefail
E=$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)
OUT=$E/nachweis/gui
mkdir -p "$OUT"
BIN=${QS_BIN:-$E/target/debug/querencia-sitzung}
[ -x "$BIN" ] || { echo "Programm fehlt: $BIN (cargo build)"; exit 2; }

lauf() {
    local sz=$1 ausgaenge=${2:-1} limit=${3:-240}
    local PROFIL=$E/gui-test/profil-${sz%.sh}
    [ -d "$E/gui-test" ] && rm -rf "$PROFIL"
    mkdir -p "$PROFIL/data/applications" "$PROFIL/data/icons"
    cp "$E/data/org.querencia.Sitzung.desktop" "$PROFIL/data/applications/"
    cp -r "$E/data/icons/hicolor" "$PROFIL/data/icons/"
    cat >"$PROFIL/env.sh" <<X
BIN=$BIN
AGENT=${QS_AGENT:-$(dirname "$BIN")/querencia-polkit-agent}
Q=$E/werkzeug/bin/qeingabe
QA=$E/werkzeug/bin/qausgabe
OUT=$OUT
EINGABE=$E/werkzeug/eingabe.sh
QS_GROESSE=${QS_GROESSE:-}
X
    echo "== Szenario $sz ($ausgaenge Ausgang/Ausgänge, Limit $limit s)"
    QS_OHNE_SYSTEM=${QS_OHNE_SYSTEM:-1} LANG=en_US.UTF-8 LANGUAGE=${QS_SPRACHE:-de} \
        python3 -I "$E/werkzeug/isoliert.py" "$PROFIL" "$E/werkzeug/$sz" "$limit" "$ausgaenge" | tee "$OUT/status-${sz%.sh}.txt" || true
    cp "$PROFIL"/logs/lauf-*.log "$OUT/labwc-${sz%.sh}.log" 2>/dev/null || true
    if [ "$sz" = szenario-abmelden.sh ]; then
        # labwc muss von selbst geendet haben (Status beendet), durch --exit.
        if grep -q 'Status: beendet' "$OUT/status-szenario-abmelden.txt" && grep -q -- '--exit beendet mit exit status: 0' "$OUT/a01.log"; then
            echo "PASS A01 --abmelden ohne Trockenlauf beendet den isolierten labwc über $(grep -o '/[^ ]*labwc --exit' "$OUT/a01.log" | head -1) (Countdown 2 s)" | tee -a "$OUT/ergebnis.txt"
        else
            echo "FAIL A01 Abmelden: $(cat "$OUT/status-szenario-abmelden.txt") / $(tr '\n' ';' <"$OUT/a01.log")" | tee -a "$OUT/ergebnis.txt"
        fi
    fi
}

if [ $# -ge 1 ]; then
    lauf "$1" "${2:-1}" "${3:-240}"
else
    : >"$OUT/ergebnis.txt"
    lauf szenario-dialog.sh 1 300
    lauf szenario-sperre.sh 2 300
    lauf szenario-leerlauf.sh 1 180
    if [ "${QS_OHNE_SYSTEM:-1}" = 1 ]; then
        echo "BLOCKED P01–P09 polkit-Szenario registriert den Agenten bei der echten logind-Sitzung – ausgelassen (QS_OHNE_SYSTEM=1, Auflage G2-AH)" | tee -a "$OUT/ergebnis.txt"
    else
        lauf szenario-polkit.sh 1 150
    fi
    lauf szenario-abmelden.sh 1 60
fi
echo
echo "== Gesamt"
cat "$OUT/ergebnis.txt"
echo "Ergebnis: $(grep -c '^PASS' "$OUT/ergebnis.txt") PASS, $(grep -c '^FAIL' "$OUT/ergebnis.txt") FAIL, $(grep -c '^BLOCKED' "$OUT/ergebnis.txt") BLOCKED" | tee -a "$OUT/ergebnis.txt"
