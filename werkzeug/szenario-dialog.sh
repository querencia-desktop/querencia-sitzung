#!/bin/bash
# Dialoge: Öffnen, Tastatur, Maus, Trockenlauf jeder Schaltfläche,
# Countdown, Fehler, Auflösungen 1024×768 und 1920×1080.
. "$(dirname "$0")/gemeinsam.sh"
export QS_TESTPASSWORT=querencia-test

# D01 öffnet, mittig
start d01 --dialog
kill -0 $P 2>/dev/null && ok "D01 --dialog öffnet ein Fenster" || nok "D01 --dialog öffnet nicht"
q "move 640 600" "wait 200"; shot d01-dialog-1280x720
$MITTE "$OUT/d01-dialog-1280x720.png" >"$OUT/d01-mitte.txt" && ok "D01b Dialog mittig: $(cat "$OUT/d01-mitte.txt")" || nok "D01b nicht mittig: $(cat "$OUT/d01-mitte.txt")"
# D02 Esc bricht ab
q "key Escape" "wait 800"; ende
[ $RC = 0 ] && grep -q '^querencia-sitzung: abgebrochen' "$OUT/d01.log" && ok "D02 Esc = Abbrechen (Exit 0)" || nok "D02 Esc (Exit $RC)"
# D03 Enter auf Standardfokus = Abbrechen
start d03 --dialog; q "key Return" "wait 800"; ende
[ $RC = 0 ] && grep -q 'abgebrochen' "$OUT/d03.log" && ok "D03 Enter auf Standardfokus Abbrechen" || nok "D03 Enter (Exit $RC)"
# D04 Pfeil rechts + Enter = Abmelden (Trockenlauf)
start d04 --dialog; q "key Right" "wait 400"; shot d04-fokus-abmelden; q "key Return" "wait 800"; ende
[ $RC = 0 ] && grep -q 'TROCKENLAUF: .*labwc --exit' "$OUT/d04.log" && ok "D04 Pfeil rechts, Enter: Abmelden im Trockenlauf (labwc --exit nur protokolliert)" || nok "D04 Abmelden (Exit $RC)"
# D05 Pfeil links + Enter = Herunterfahren
start d05 --dialog; q "key Left" "wait 300" "key Return" "wait 1500"; ende
[ $RC = 0 ] && grep -q 'logind CanPowerOff: ' "$OUT/d05.log" && grep -q 'TROCKENLAUF: logind PowerOff' "$OUT/d05.log" && ok "D05 Pfeil links, Enter: Herunterfahren im Trockenlauf ($(grep -o 'CanPowerOff: [a-z]*' "$OUT/d05.log"))" || nok "D05 Herunterfahren (Exit $RC)"
# D06 Links, Links, Enter = Neu starten
start d06 --dialog; q "key Left" "key Left" "wait 300" "key Return" "wait 1500"; ende
[ $RC = 0 ] && grep -q 'TROCKENLAUF: logind Reboot' "$OUT/d06.log" && ok "D06 Pfeil links ×2, Enter: Neu starten im Trockenlauf ($(grep -o 'CanReboot: [a-z]*' "$OUT/d06.log"))" || nok "D06 Neu starten (Exit $RC)"
# D07 Maus: Neu starten anklicken
start d07 --dialog; q "move 524 421" "wait 300" "click" "wait 1500"; ende
[ $RC = 0 ] && grep -q 'TROCKENLAUF: logind Reboot' "$OUT/d07.log" && ok "D07 Maus: Klick auf „Neu starten“ im Trockenlauf" || nok "D07 Maus Neu starten (Exit $RC)"
# D08 Mnemonik Alt+M = Abmelden
start d08 --dialog; q "chord alt m" "wait 800"; ende
[ $RC = 0 ] && grep -q 'TROCKENLAUF: .*labwc --exit' "$OUT/d08.log" && ok "D08 Alt+M: Abmelden im Trockenlauf" || nok "D08 Alt+M (Exit $RC)"
# D09 Maus: Bildschirm sperren → Sperre, Entsperren mit Testpasswort
start d09 --dialog; q "move 362 421" "wait 300" "click" "wait 2500"
shot d09-sperre-aus-dialog
grep -q 'Sitzung gesperrt' "$OUT/d09.log" && ok "D09 Maus: „Bildschirm sperren“ sperrt (ext-session-lock)" || nok "D09 Sperre aus Dialog"
q "type querencia-test" "key Return" "wait 1500"; ende
[ $RC = 0 ] && ok "D09b Entsperren mit Testpasswort, Exit 0" || nok "D09b Entsperren (Exit $RC)"
# D10 --abmelden mit Countdown 3
START_WARTEN=1.5 start d10 --abmelden --countdown 3; shot d10-abmelden-countdown; sleep 3; ende
[ $RC = 0 ] && grep -q 'TROCKENLAUF: .*labwc --exit' "$OUT/d10.log" && ok "D10 --abmelden --countdown 3 löst nach Ablauf aus (Trockenlauf)" || nok "D10 Countdown (Exit $RC)"
# D11 --neustart: Rückfrage, Esc
start d11 --neustart; shot d11-neustart-rueckfrage; q "key Escape" "wait 800"; ende
[ $RC = 0 ] && grep -q abgebrochen "$OUT/d11.log" && ok "D11 --neustart Rückfrage, Esc bricht ab" || nok "D11 (Exit $RC)"
# D12 --herunterfahren --kein-countdown: Rechts, Enter
start d12 --herunterfahren --kein-countdown; shot d12-herunterfahren-rueckfrage; q "key Right" "wait 300" "key Return" "wait 1500"; ende
[ $RC = 0 ] && grep -q 'TROCKENLAUF: logind PowerOff' "$OUT/d12.log" && ok "D12 --herunterfahren --kein-countdown: Rechts, Enter im Trockenlauf" || nok "D12 (Exit $RC)"
# D13 --sperren Rückfrage → Sperre → Entsperren
start d13 --sperren; shot d13-sperren-rueckfrage; q "key Right" "wait 300" "key Return" "wait 2500"; shot d13-gesperrt
grep -q 'Sitzung gesperrt' "$OUT/d13.log" && ok "D13 --sperren: Rückfrage, dann Sperre" || nok "D13 Sperre"
q "type querencia-test" "key Return" "wait 1500"; ende
[ $RC = 0 ] && ok "D13b Entsperren nach Rückfrage-Sperre" || nok "D13b (Exit $RC)"
# D14 Fehler der Befehlszeile
"$BIN" --unsinn >"$OUT/d14.log" 2>&1; rc1=$?
"$BIN" --dialog --sofort >>"$OUT/d14.log" 2>&1; rc2=$?
"$BIN" >>"$OUT/d14.log" 2>&1; rc3=$?
[ $rc1 = 2 ] && [ $rc2 = 2 ] && [ $rc3 = 2 ] && ok "D14 falsche Argumente: Exit 2 mit Meldung" || nok "D14 Argumente ($rc1 $rc2 $rc3)"
"$BIN" --version >"$OUT/d14-version.txt" 2>&1 && grep -q '^querencia-sitzung 0.1.1' "$OUT/d14-version.txt" && ok "D14b --version: $(cat "$OUT/d14-version.txt")" || nok "D14b --version"
# D15 zweiter Aufruf holt den offenen Dialog nach vorn (eine Instanz)
start d15 --dialog; "$BIN" --dialog >"$OUT/d15b.log" 2>&1; rc=$?
kill -0 $P 2>/dev/null && [ $rc = 0 ] && ok "D15 zweiter --dialog-Aufruf endet sofort (Exit 0), erster Dialog bleibt" || nok "D15 zweite Instanz (Exit $rc)"
q "key Escape" "wait 800"; ende
# D16 1024×768
"$QA" 1024x768 >"$OUT/d16-qausgabe.log" 2>&1; sleep 1.5
start d16 --dialog; q "move 512 700" "wait 200"; shot d16-dialog-1024x768
$MITTE "$OUT/d16-dialog-1024x768.png" >"$OUT/d16-mitte.txt" && ok "D16 1024×768: $(cat "$OUT/d16-mitte.txt")" || nok "D16 1024×768: $(cat "$OUT/d16-mitte.txt")"
q "key Escape" "wait 800"; ende
START_WARTEN=1.5 start d16b --herunterfahren; shot d16-herunterfahren-1024x768; q "key Escape" "wait 800"; ende
# D17 1920×1080
"$QA" 1920x1080 >"$OUT/d17-qausgabe.log" 2>&1; sleep 1.5
start d17 --dialog; q "move 960 1000" "wait 200"; shot d17-dialog-1920x1080
$MITTE "$OUT/d17-dialog-1920x1080.png" >"$OUT/d17-mitte.txt" && ok "D17 1920×1080: $(cat "$OUT/d17-mitte.txt")" || nok "D17 1920×1080: $(cat "$OUT/d17-mitte.txt")"
q "key Escape" "wait 800"; ende
START_WARTEN=1.5 start d17b --abmelden; shot d17-abmelden-1920x1080; q "key Escape" "wait 800"; ende
cat "$OUT"/d*.log | grep -iE 'critical|warning' | grep -v -E 'vkGetPhysicalDevice|libEGL' | head -20 >"$OUT/d-gtk-meldungen.txt"
[ -s "$OUT/d-gtk-meldungen.txt" ] && echo "HINW GTK-Meldungen (Dialog): $(wc -l <"$OUT/d-gtk-meldungen.txt")" | tee -a "$R"
touch "$QS_PROFILE/logs/FERTIG"
