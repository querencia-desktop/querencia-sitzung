#!/bin/bash
# Bildschirm aus nach der Sperre (0.1.6), nur im isolierten labwc mit
# eigenem Sitzungsbus. Der Leerlaufdienst sperrt selbst erst nach 300 s;
# gesperrt wird über org.freedesktop.ScreenSaver.Lock (bzw. wie vom
# Tastenkürzel mit --sperren --sofort). BildschirmAus steht in der
# sitzung.ini des Profils (4 s). Ob der Ausgang an ist, misst grim: Ein
# abgeschalteter Ausgang lässt sich nicht fotografieren. labwc bestätigt den
# Wechsel außerdem an die Steuerung des Dienstes („Ausgang meldet aus“).
# wlr-output-management meldet ihn nicht (labwc schickt kein neues
# „enabled“, gemessen). Belege: $OUT/b*.log (Dienst), Fotos.
. "$(dirname "$0")/gemeinsam.sh"
: >"$R"
W=$(dirname "$EINGABE")
CLIENT="python3 -I $W/schoner-client.py"
GF="python3 -I $W/grundfarbe.py"
case ${DBUS_SESSION_BUS_ADDRESS:-} in
"" | *"/run/user/"*) nok "Sicherheitsnetz: Sitzungsbus $DBUS_SESSION_BUS_ADDRESS ist nicht der Wegwerf-Bus"; touch "$QS_PROFILE/logs/FERTIG"; exit 1 ;;
esac
S="busctl --user --timeout=5"
aktiv() { $S call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver GetActive 2>&1; }
sperren() { $S call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver Lock >>"$OUT/b-busctl.txt" 2>&1; }
INI=$XDG_CONFIG_HOME/querencia/sitzung.ini
mkdir -p "$(dirname "$INI")"
export QS_TESTPASSWORT=querencia-test

# Stand des Ausgangs: an, wenn grim ihn fotografieren kann
stand() { grim -o HEADLESS-1 - >/dev/null 2>"$OUT/b-grim.txt" && echo an || echo aus; }
# warte_stand an|aus SEK: bis der Beobachter den Stand meldet; Zeit in $DT
warte_stand() { local i t0; t0=$(date +%s.%N); for ((i = 0; i < $2 * 10; i++)); do [ "$(stand)" = "$1" ] && { DT=$(python3 -c "print(round($(date +%s.%N)-$t0,1))"); return 0; }; sleep 0.1; done; DT=-; return 1; }
im_log() { local i; for ((i = 0; i < $3 * 10; i++)); do grep -q -- "$2" "$1" && return 0; sleep 0.1; done; return 1; }
anzahl() { grep -c -- "$2" "$1"; }
entsperren() {
    sleep 1
    q "type querencia-test" "key Return" "wait 800"
    local i; for ((i = 0; i < 40; i++)); do [ "$(aktiv)" = "b false" ] && return 0; sleep 0.1; done
    echo "INFO entsperren: GetActive bleibt true" >>"$R"; return 1
}
dienst() { "$BIN" --leerlauf-sekunden 300 >"$OUT/$1.log" 2>&1 & L=$!; im_log "$OUT/$1.log" 'auf dem Sitzungsbus übernommen' 5; q "move 100 100" "wait 100"; }
dienst_ende() { kill "$L" 2>/dev/null; wait "$L" 2>/dev/null; }

printf '[Sperre]\nBildschirmAus=4\n' >"$INI"
dienst b00
D=$OUT/b00.log
[ "$(stand)" = an ] && grep -q "Bildschirm: aus nach 4 s gesperrt ohne Eingabe ($INI; 1 Ausgang" "$D" &&
    ok "B01 Start: Ausgang an, Dienst nennt 4 s aus sitzung.ini und 1 Ausgang" || nok "B01 Stand=$(stand) $(grep Bildschirm "$D" | tr '\n' ';')"

# B02–B05: sperren, nach 4 s ohne Eingabe aus, Eingabe → an, wieder aus
sperren; sleep 2
[ "$(aktiv)" = "b true" ] && [ "$(stand)" = an ] && ok "B02 gesperrt (Lock), nach 2 s noch an" || nok "B02 aktiv=$(aktiv) Stand=$(stand)"
warte_stand aus 6 && grep -q 'Ausgang/Ausgänge aus (gesperrt, keine Eingabe)' "$D" &&
    ok "B03 Ausgang aus nach weiteren ${DT} s (Soll gesamt ≈ 4 s nach dem Sperren)" || nok "B03 Stand=$(stand) $(tail -3 "$D" | tr '\n' ';')"
cp "$OUT/b-grim.txt" "$OUT/b03-grim-aus.txt"
grep -q 'Bildschirm: Ausgang meldet aus' "$D" && ok "B03b labwc bestätigt „aus“ an die Steuerung; grim: $(head -c 120 "$OUT/b03-grim-aus.txt" | tr '\n' ' ')" || nok "B03b keine Bestätigung"
q "move 300 300" "wait 100"
warte_stand an 3 && grep -q 'Ausgänge an (Eingabe)' "$D" && [ "$(aktiv)" = "b true" ] &&
    ok "B04 Mausbewegung: an nach ${DT} s, Sitzung bleibt gesperrt" || nok "B04 Stand=$(stand) aktiv=$(aktiv)"
sleep 1.5; shot b04-an-gesperrt
m=$($GF "$OUT/b04-an-gesperrt.png" 400 200); echo "INFO b04: $m" >>"$R"
[[ $m != *"Grund #000000"* ]] && ok "B04b Sperrbildschirm nach dem Einschalten sichtbar (Bild b04-an-gesperrt: $m)" || nok "B04b schwarz: $m"
warte_stand aus 7 && ok "B05 ohne Eingabe wieder aus nach ${DT} s" || nok "B05 Stand=$(stand)"
# B06: Entsperren aus dem ausgeschalteten Zustand
q "move 320 320" "wait 100"; warte_stand an 3
entsperren && warte_stand an 2 && grep -q 'Bildschirm: Ausgänge an\|entsperrt' "$D" &&
    ok "B06 entsperrt mit Testpasswort, Ausgang an" || nok "B06 aktiv=$(aktiv) Stand=$(stand)"
n=$(anzahl "$D" 'Ausgänge aus'); sleep 7
[ "$(stand)" = an ] && [ "$(anzahl "$D" 'Ausgänge aus')" = "$n" ] &&
    ok "B07 entsperrt: 7 s ohne Eingabe, Ausgang bleibt an" || nok "B07 Stand=$(stand)"
shot b07-entsperrt

# B08: Hemmung (Video) hält den Ausgang an, UnInhibit zählt von vorn
mkfifo "$QS_PROFILE/c1.in"
$CLIENT <"$QS_PROFILE/c1.in" >"$OUT/b-client.txt" 2>&1 & C1=$!
exec 7>"$QS_PROFILE/c1.in"
echo "inhibit Firefox video-playing /ScreenSaver" >&7
for ((i = 0; i < 30; i++)); do grep -q '^cookie ' "$OUT/b-client.txt" && break; sleep 0.1; done
COOKIE=$(sed -n 's/^cookie //p' "$OUT/b-client.txt")
sperren; sleep 7
[ -n "$COOKIE" ] && [ "$(aktiv)" = "b true" ] && [ "$(stand)" = an ] && grep -q 'Bildschirm bleibt an: 1 Hemmung(en): Firefox (video-playing)' "$D" &&
    ok "B08 Hemmung (Cookie $COOKIE): gesperrt, nach 7 s noch an, protokolliert" || nok "B08 Cookie=$COOKIE Stand=$(stand) $(tail -3 "$D" | tr '\n' ';')"
echo "uninhibit $COOKIE /ScreenSaver" >&7
warte_stand aus 7 && grep -q 'Bildschirm: aus nach 4 s ohne Eingabe (letzte Hemmung aufgehoben)' "$D" &&
    ok "B09 UnInhibit: zählt von vorn, aus nach ${DT} s" || nok "B09 Stand=$(stand)"
echo "inhibit Player video /org/freedesktop/ScreenSaver" >&7
warte_stand an 3 && grep -q 'Ausgänge an (Hemmung)' "$D" &&
    ok "B10 neue Hemmung bei ausgeschaltetem Ausgang: an nach ${DT} s" || nok "B10 Stand=$(stand)"
exec 7>&-; wait $C1 2>/dev/null
q "move 330 330" "wait 100"; entsperren || nok "B10 entsperren"
# B11: SimulateUserActivity bei ausgeschaltetem Ausgang
sperren; warte_stand aus 8
$S call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver SimulateUserActivity >>"$OUT/b-busctl.txt" 2>&1
warte_stand an 3 && grep -q 'Ausgänge an (SimulateUserActivity)' "$D" &&
    ok "B11 SimulateUserActivity: an nach ${DT} s, zählt von vorn" || nok "B11 Stand=$(stand)"
warte_stand aus 7 && ok "B11b danach wieder aus nach ${DT} s" || nok "B11b Stand=$(stand)"
q "move 340 340" "wait 100"; warte_stand an 3; entsperren || nok "B11 entsperren"

# B12: Sperre vom Tastenkürzel (nicht vom Dienst gestartet), bemerkt in ≤ 5 s
"$BIN" --sperren --sofort >"$OUT/b12-sperre.log" 2>&1 & SP=$!
sleep 2
warte_stand aus 12 && grep -q 'Bildschirm: aus nach 4 s ohne Eingabe (gesperrt)' "$D" &&
    ok "B12 Sperre vom Tastenkürzel: aus nach ${DT} s (Erkennung ≤ 5 s + 4 s)" || nok "B12 Stand=$(stand)"
q "move 350 350" "wait 100"; warte_stand an 3; entsperren || nok "B12 entsperren"; wait $SP 2>/dev/null
dienst_ende

# B13: BildschirmAus=0 – nie aus
printf '[Sperre]\nBildschirmAus=0\n' >"$INI"
dienst b13
grep -q "Bildschirm: bleibt nach der Sperre an ($INI)" "$OUT/b13.log" && sperren && sleep 8 && [ "$(aktiv)" = "b true" ] && [ "$(stand)" = an ] &&
    ok "B13 BildschirmAus=0: gesperrt, nach 8 s noch an" || nok "B13 Stand=$(stand) $(grep Bildschirm "$OUT/b13.log" | tr '\n' ';')"
entsperren || nok "B13 entsperren"
dienst_ende
# B14: ohne sitzung.ini gilt die Vorgabe 120 s
rm -f "$INI"
dienst b14
grep -q 'Bildschirm: aus nach 120 s gesperrt ohne Eingabe (Vorgabe' "$OUT/b14.log" &&
    ok "B14 ohne sitzung.ini: Vorgabe 120 s" || nok "B14 $(grep Bildschirm "$OUT/b14.log" | tr '\n' ';')"
dienst_ende

# B15: am Ende an, keine Zombies, Beobachter lief durch
z=$(ps -o pid=,stat= --ppid $$ 2>/dev/null | awk '$2 ~ /^Z/' | wc -l)
[ "$(stand)" = an ] && [ "$z" = 0 ] && ok "B15 am Ende Ausgang an, keine Zombie-Kinder" || nok "B15 Stand=$(stand) Zombies=$z"
grep -h 'Bildschirm' "$OUT"/b00.log | sed 's/^/INFO Dienst: /' >>"$R"
echo "Ergebnis Bildschirm aus: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$QS_PROFILE/logs/FERTIG"
