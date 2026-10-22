#!/bin/bash
# Dialog „Herunterfahren“ mit „Neu starten“ (0.1.7): Reihenfolge Abbrechen,
# Neu starten, Herunterfahren; Fokus auf Abbrechen, Vorgabe und Countdown
# beim Herunterfahren; „Neu starten“ löst sofort den Neustart aus. Beide
# Wege gehen wie --neustart/--herunterfahren an logind, hier an
# logind-ersatz.py auf einem eigenen Wegwerf-Systembus
# (DBUS_SYSTEM_BUS_ADDRESS); das echte logind wird nie gefragt, und
# QS_TROCKENLAUF=1 gilt zusätzlich. Fotos hell und dunkel (Settings-Portal-
# Ersatz, gui-pruefung.sh setzt QS_PORTAL=1) in LANGUAGE und QS_GROESSE.
. "$(dirname "$0")/gemeinsam.sh"
: >"$R"
W=$(dirname "$EINGABE")
SPRACHE=${LANGUAGE%%:*}
GROESSE=${QS_GROESSE:-1280x720}
GF="python3 -I $W/grundfarbe.py"
# Bilder je Sprache und Größe
bild() { grim -o HEADLESS-1 "$OUT/$1-$SPRACHE-$GROESSE.png"; }
weg() { q "move 1270 710" "wait 200"; }
echo "INFO LANG=$LANG LANGUAGE=$LANGUAGE Ausgang $GROESSE" >>"$R"

# Thema und Portal-Ersatz wie szenario-dunkel.sh
schrift=$(sed -n 's/^gtk-font-name=//p' "$XDG_CONFIG_HOME/gtk-4.0/settings.ini" 2>/dev/null)
[ -n "$schrift" ] && gsettings set org.gnome.desktop.interface font-name "$schrift"
gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch
gsettings set org.gnome.desktop.interface color-scheme default
python3 -I "$W/portal-ersatz.py" "$QS_PROFILE/logs/portal-bereit" >>"$OUT/h-portal.log" 2>&1 &
for ((i = 0; i < 50; i++)); do [ -f "$QS_PROFILE/logs/portal-bereit" ] && break; sleep 0.1; done
[ -f "$QS_PROFILE/logs/portal-bereit" ] || { nok "Portal-Ersatz startet nicht"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
schema() {
    case $1 in dunkel) gsettings set org.gnome.desktop.interface color-scheme prefer-dark ;;
    *) gsettings set org.gnome.desktop.interface color-scheme default ;; esac
    sleep 0.5
}

# Wegwerf-Systembus mit logind-Ersatz
SYSBUS=$XDG_RUNTIME_DIR/systembus-ersatz
L=$OUT/h-logind-ersatz.log
: >"$L"
dbus-daemon --session --nofork --nopidfile --address="unix:path=$SYSBUS" >>"$OUT/h-systembus.log" 2>&1 &
BUSPID=$!
for ((i = 0; i < 50; i++)); do [ -S "$SYSBUS" ] && break; sleep 0.1; done
export DBUS_SYSTEM_BUS_ADDRESS="unix:path=$SYSBUS"
python3 -I "$W/logind-ersatz.py" "$DBUS_SYSTEM_BUS_ADDRESS" "$L" "$QS_PROFILE/logs/logind-bereit" 2>>"$OUT/h-systembus.log" &
LPID=$!
for ((i = 0; i < 50; i++)); do [ -f "$QS_PROFILE/logs/logind-bereit" ] && break; sleep 0.1; done
[ -f "$QS_PROFILE/logs/logind-bereit" ] || { nok "logind-Ersatz startet nicht"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
aufraeumen() { kill "$LPID" "$BUSPID" 2>/dev/null; wait "$LPID" "$BUSPID" 2>/dev/null; }
# Sicherheitsnetz: Systembus des Programms ist der Wegwerf-Bus im
# isolierten Laufzeitordner.
[[ $DBUS_SYSTEM_BUS_ADDRESS == unix:path=*/qed-runtime-*/systembus-ersatz ]] ||
    { nok "Sicherheitsnetz: DBUS_SYSTEM_BUS_ADDRESS $DBUS_SYSTEM_BUS_ADDRESS"; aufraeumen; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
systembus_pruefen() {
    tr '\0' '\n' <"/proc/$P/environ" | grep -qx "DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SYSTEM_BUS_ADDRESS" ||
        { nok "Sicherheitsnetz: $1 ohne Wegwerf-Systembus"; kill "$P"; aufraeumen; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
}
hstart() { local log=$1; start "$@"; systembus_pruefen "$log"; }
# Zugriffstasten der Sprache; Mitte von „Neu starten“ bei 1280×720
# (gemessen in h02-hell-<sprache>-1280x720.png)
case $SPRACHE in de) T_NEU=t T_AUS=h H01_X=632 H01_Y=427 ;; *) T_NEU=r T_AUS=s H01_X=720 H01_Y=415 ;; esac
# prueft LOG: Neustart bzw. Herunterfahren im Trockenlauf über logind
neustart_ok() { grep -q 'Aktion: Neu starten' "$OUT/$1.log" && grep -q 'logind CanReboot: yes' "$OUT/$1.log" &&
    grep -q 'TROCKENLAUF: logind Reboot(interactive=true) nicht aufgerufen' "$OUT/$1.log" && ! grep -q 'Aktion: Herunterfahren' "$OUT/$1.log"; }
aus_ok() { grep -q 'Aktion: Herunterfahren' "$OUT/$1.log" && grep -q 'logind CanPowerOff: yes' "$OUT/$1.log" &&
    grep -q 'TROCKENLAUF: logind PowerOff(interactive=true) nicht aufgerufen' "$OUT/$1.log" && ! grep -q 'Aktion: Neu starten' "$OUT/$1.log"; }

# H01 Maus (1280×720, Rasterkoordinaten von qeingabe): Klick auf „Neu starten“
if [ "$GROESSE" = 1280x720 ]; then
    hstart h01 --herunterfahren; q "move $H01_X $H01_Y" "wait 300"; bild h01-maus-vorher; q "click" "wait 1500"; ende
    [ $RC = 0 ] && neustart_ok h01 && ok "H01 Maus: Klick auf „Neu starten“ startet neu (Trockenlauf, logind-Ersatz)" || nok "H01 Maus (Exit $RC): $(tr '\n' ';' <"$OUT/h01.log")"
fi
"$QA" "$GROESSE" >"$OUT/h-qausgabe.log" 2>&1 || true
sleep 1.5

# H02 hell: drei Knöpfe, Foto, Esc
schema hell
hstart h02 --herunterfahren; weg; bild h02-hell
q "key Escape" "wait 800"; ende
[ $RC = 0 ] && grep -q '^querencia-sitzung: abgebrochen' "$OUT/h02.log" && ok "H02 --herunterfahren öffnet (Bild h02-hell), Esc bricht ab" || nok "H02 (Exit $RC)"
m=$($GF "$OUT/h02-hell-$SPRACHE-$GROESSE.png"); echo "INFO h02-hell: $m" >>"$R"
[[ $m == *" hell, "* ]] && ok "H02b Dialog hell ($m)" || nok "H02b soll hell ($m)"
# H03 Enter auf dem Fokus „Abbrechen“ = Abbrechen
hstart h03 --herunterfahren; q "key Return" "wait 800"; ende
[ $RC = 0 ] && grep -q abgebrochen "$OUT/h03.log" && ! grep -q 'Aktion:' "$OUT/h03.log" && ok "H03 Enter auf dem Fokus „Abbrechen“ bricht ab" || nok "H03 (Exit $RC)"
# H04 Pfeil rechts, Enter = Neu starten
hstart h04 --herunterfahren; q "key Right" "wait 300"; bild h04-fokus-neustart; q "key Return" "wait 1500"; ende
[ $RC = 0 ] && neustart_ok h04 && ok "H04 Pfeil rechts, Enter: Neu starten (Bild h04-fokus-neustart)" || nok "H04 (Exit $RC): $(tr '\n' ';' <"$OUT/h04.log")"
# H05 Pfeil rechts ×2, Enter = Herunterfahren
hstart h05 --herunterfahren; q "key Right" "key Right" "wait 300"; bild h05-fokus-herunterfahren; q "key Return" "wait 1500"; ende
[ $RC = 0 ] && aus_ok h05 && ok "H05 Pfeil rechts ×2, Enter: Herunterfahren (Bild h05-fokus-herunterfahren)" || nok "H05 (Exit $RC): $(tr '\n' ';' <"$OUT/h05.log")"
# H06/H07 Zugriffstasten
hstart h06 --herunterfahren; q "chord alt $T_NEU" "wait 1500"; ende
[ $RC = 0 ] && neustart_ok h06 && ok "H06 Alt+$T_NEU: Neu starten" || nok "H06 Alt+$T_NEU (Exit $RC): $(tr '\n' ';' <"$OUT/h06.log")"
hstart h07 --herunterfahren; q "chord alt $T_AUS" "wait 1500"; ende
[ $RC = 0 ] && aus_ok h07 && ok "H07 Alt+$T_AUS: Herunterfahren" || nok "H07 Alt+$T_AUS (Exit $RC): $(tr '\n' ';' <"$OUT/h07.log")"
# H08 Countdown 3 s läuft auf Herunterfahren
START_WARTEN=1.2 hstart h08 --herunterfahren --countdown 3; bild h08-countdown; sleep 3; ende
[ $RC = 0 ] && aus_ok h08 && ok "H08 --countdown 3 löst Herunterfahren aus, nicht Neu starten (Bild h08-countdown)" || nok "H08 (Exit $RC): $(tr '\n' ';' <"$OUT/h08.log")"
# H09 Countdown 30 s, „Neu starten“ gewählt: sofort Neustart
hstart h09 --herunterfahren --countdown 30; t0=$(date +%s.%N); q "key Right" "key Return" "wait 1500"; ende; t1=$(date +%s.%N)
d=$(python3 -c "print(round($t1-$t0,1))")
[ $RC = 0 ] && neustart_ok h09 && python3 -c "import sys; sys.exit(0 if $d < 5 else 1)" && ok "H09 Countdown 30 s, „Neu starten“ gewählt: Neustart nach ${d} s" || nok "H09 (Exit $RC, ${d} s): $(tr '\n' ';' <"$OUT/h09.log")"
# H10 --neustart bleibt: Abbrechen, Neu starten
hstart h10 --neustart; weg; bild h10-neustart; q "key Right" "wait 300" "key Return" "wait 1500"; ende
[ $RC = 0 ] && neustart_ok h10 && ok "H10 --neustart unverändert: Pfeil rechts, Enter startet neu (Bild h10-neustart)" || nok "H10 (Exit $RC): $(tr '\n' ';' <"$OUT/h10.log")"
# H11 dunkel: Foto, Esc
schema dunkel
hstart h11 --herunterfahren; weg; bild h11-dunkel
q "key Escape" "wait 800"; ende
m=$($GF "$OUT/h11-dunkel-$SPRACHE-$GROESSE.png"); echo "INFO h11-dunkel: $m" >>"$R"
[ $RC = 0 ] && grep -q abgebrochen "$OUT/h11.log" && [[ $m == *" dunkel, "* ]] && ok "H11 dunkel (Bild h11-dunkel, $m), Esc bricht ab" || nok "H11 (Exit $RC, $m)"
START_WARTEN=1.2 hstart h11b --herunterfahren --countdown 30; weg; bild h11b-dunkel-countdown; q "key Escape" "wait 800"; ende
schema hell
# H12 logind-Ersatz: nur Can*-Fragen, nie Reboot/PowerOff
nr=$(grep -c '^AUFRUF CanReboot' "$L"); np=$(grep -c '^AUFRUF CanPowerOff' "$L")
soll_r=4; [ "$GROESSE" = 1280x720 ] && soll_r=5
if ! grep -qE '^AUFRUF (Reboot|PowerOff)' "$L" && [ "$nr" = $soll_r ] && [ "$np" = 3 ]; then
    ok "H12 logind-Ersatz: $nr× CanReboot, $np× CanPowerOff, kein Reboot/PowerOff (Trockenlauf)"
else
    nok "H12 logind-Ersatz: CanReboot $nr (soll $soll_r), CanPowerOff $np (soll 3), $(grep -cE '^AUFRUF (Reboot|PowerOff)' "$L") Reboot/PowerOff"
fi
aufraeumen
cat "$OUT"/h*.log | grep -iE 'critical|warning' | grep -v -E 'vkGetPhysicalDevice|libEGL' | head -20 >"$OUT/h-gtk-meldungen.txt"
[ -s "$OUT/h-gtk-meldungen.txt" ] && echo "HINW GTK-Meldungen: $(wc -l <"$OUT/h-gtk-meldungen.txt")" | tee -a "$R"
echo "Ergebnis $GROESSE $SPRACHE: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$QS_PROFILE/logs/FERTIG"
