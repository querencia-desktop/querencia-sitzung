#!/bin/bash
# Sprache der Querencia Sitzung 0.1.2 (G2-AH) in LANGUAGE (gui-pruefung.sh
# QS_SPRACHE): eingeblendeter Katalog (/proc/<pid>/maps), Dialog „Sitzung
# beenden“, Rückfragen Abmelden (mit Countdown), Neu starten, Herunterfahren,
# Bildschirm sperren – jeweils nur öffnen und mit Esc abbrechen –, dazu die
# Zugriffstaste „Abmelden“ der Sprache im Trockenlauf (isolierter labwc).
# Auflösung aus QS_GROESSE (Standard 1280x720).
. "$(dirname "$0")/gemeinsam.sh"
: >"$R"
SPRACHE=${LANGUAGE%%:*}
GROESSE=${QS_GROESSE:-1280x720}
B=${GROESSE%x*}; H=${GROESSE#*x}
shot() { grim -o HEADLESS-1 "$OUT/$1-$SPRACHE-$GROESSE.png"; }
"$QA" "$GROESSE" >"$OUT/qausgabe.log" 2>&1 || true
sleep 1.5
echo "INFO LANG=$LANG LANGUAGE=$LANGUAGE Ausgang $GROESSE" >>"$R"
# qeingabe rechnet Zeigerkoordinaten im 1280x720-Raster; Zeiger aus dem Weg
weg() { q "move 1270 710" "wait 200"; }

# 1 Dialog „Sitzung beenden“, Katalog, Esc
start k1 --dialog
KAT=$(grep -o '[^ ]*querencia-sitzung.mo' "/proc/$P/maps" 2>/dev/null | head -1)
echo "INFO Katalog: ${KAT:-keiner}" >>"$R"
case $SPRACHE in
de) [ -n "$KAT" ] && ok "de: Katalog eingeblendet" || nok "de: kein Katalog eingeblendet" ;;
*) [ -z "$KAT" ] && ok "$SPRACHE: kein Katalog eingeblendet" || nok "$SPRACHE: Katalog eingeblendet" ;;
esac
weg; shot k1-dialog
q "key Escape" "wait 800"; ende
[ $RC = 0 ] && grep -q 'abgebrochen' "$OUT/k1.log" && ok "Dialog „Sitzung beenden“ (Bild k1), Esc bricht ab" || nok "k1 (Exit $RC)"

# 2 Rückfragen nur öffnen und abbrechen
for a in abmelden neustart herunterfahren sperren; do
    START_WARTEN=1.5 start "k2-$a" "--$a"
    weg; shot "k2-$a"
    q "key Escape" "wait 800"; ende
    [ $RC = 0 ] && grep -q 'abgebrochen' "$OUT/k2-$a.log" && ok "Rückfrage --$a geöffnet (Bild k2-$a), Esc bricht ab" || nok "k2-$a (Exit $RC)"
done

# 3 Zugriffstaste „Abmelden“ der Sprache (de Ab_melden, en Log _Out) im Trockenlauf
case $SPRACHE in de) T=m ;; *) T=o ;; esac
start k3 --dialog; q "chord alt $T" "wait 800"; ende
[ $RC = 0 ] && grep -q 'TROCKENLAUF: .*labwc --exit' "$OUT/k3.log" && ok "Alt+$T löst „Abmelden“ aus (Trockenlauf, isolierter labwc)" || nok "Alt+$T (Exit $RC)"

echo "Ergebnis $GROESSE $SPRACHE: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$QS_PROFILE/logs/FERTIG"
