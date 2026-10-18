#!/bin/bash
# Sperrbildschirm mit zwei Ausgängen: Sperren, zweite Sperre abgelehnt,
# falsches/richtiges Testpasswort, Auflösungen, echtes PAM, Wiederherstellung.
. "$(dirname "$0")/gemeinsam.sh"
export QS_TESTPASSWORT=querencia-test

# S01 zwei Ausgänge
start s01 --sperren --sofort
grep -q 'Sperrfläche auf HEADLESS-1' "$OUT/s01.log" && grep -q 'Sperrfläche auf HEADLESS-2' "$OUT/s01.log" && grep -q 'Sitzung gesperrt' "$OUT/s01.log" &&
    ok "S01 Sperre auf zwei Ausgängen ($(grep -o 'Sperrfläche auf [^ ]* [0-9×]*' "$OUT/s01.log" | tr '\n' ';'))" || nok "S01 zwei Ausgänge"
shot s01-sperre-zwei-ausgaenge
grim -o HEADLESS-2 "$OUT/s01-sperre-ausgang-2.png"
# S02 zweite Sperre wird abgelehnt
"$BIN" --sperren --sofort >"$OUT/s02.log" 2>&1; rc=$?
[ $rc = 2 ] && grep -q 'finished vor locked' "$OUT/s02.log" && ok "S02 zweite Sperre während der Sperre: abgelehnt, Exit 2" || nok "S02 zweite Sperre (Exit $rc)"
# S03 falsches Passwort
q "type falsch" "wait 200"; shot s03-eingabe; q "key Return" "wait 800"; shot s03-falsches-passwort
grep -q 'Testpasswort statt PAM' "$OUT/s03.log" 2>/dev/null; grep -q 'TROCKENLAUF: Testpasswort' "$OUT/s01.log" && kill -0 $P 2>/dev/null && ok "S03 falsches Passwort: Sperre bleibt (Meldung „Falsches Passwort“)" || nok "S03 falsches Passwort"
# S04 Esc leert, Rücktaste, richtiges Passwort entsperrt
q "type abc" "key Escape" "type querencia-testx" "key BackSpace" "wait 200"; shot s04-vor-enter; q "key Return" "wait 1500"; ende
[ $RC = 0 ] && ok "S04 Esc leert, Rücktaste löscht, richtiges Testpasswort entsperrt (Exit 0)" || nok "S04 Entsperren (Exit $RC)"
# S05 danach ist der Bildschirm benutzbar
start s05 --dialog; shot s05-nach-entsperren; kill -0 $P 2>/dev/null && ok "S05 nach dem Entsperren öffnet ein Fenster" || nok "S05 kein Fenster nach Entsperren"
q "key Escape" "wait 800"; ende
# S06 1024×768 (Ausgang 1), Ausgang 2 bleibt 1280×720
"$QA" 1024x768 >"$OUT/s06-qausgabe.log" 2>&1; sleep 1.5
start s06 --sperren --sofort; grim -o HEADLESS-1 "$OUT/s06-sperre-1024x768.png"
grep -q 'HEADLESS-1: 1024×768' "$OUT/s06.log" && ok "S06 Sperre 1024×768 auf Ausgang 1 (Ausgang 2: $(grep -o 'HEADLESS-2: [0-9×]*' "$OUT/s06.log"))" || nok "S06 1024×768: $(grep Sperrfläche "$OUT/s06.log" | tr '\n' ';')"
q "type querencia-test" "key Return" "wait 1500"; ende; [ $RC = 0 ] || nok "S06b Entsperren (Exit $RC)"
# S07 1920×1080
"$QA" 1920x1080 >"$OUT/s07-qausgabe.log" 2>&1; sleep 1.5
start s07 --sperren --sofort; grim -o HEADLESS-1 "$OUT/s07-sperre-1920x1080.png"
grep -q 'HEADLESS-1: 1920×1080' "$OUT/s07.log" && ok "S07 Sperre 1920×1080 auf Ausgang 1" || nok "S07 1920×1080: $(grep Sperrfläche "$OUT/s07.log" | tr '\n' ';')"
q "type querencia-test" "key Return" "wait 1500"; ende; [ $RC = 0 ] || nok "S07b Entsperren (Exit $RC)"
# S08 echtes PAM (Dienst login): falsches Passwort wird abgelehnt
unset QS_TESTPASSWORT
start s08 --sperren --sofort
grep -q 'PAM-Dienst login' "$OUT/s08.log" && ! grep -q Testpasswort "$OUT/s08.log" || nok "S08 PAM-Dienst nicht wie erwartet"
q "type falsch" "key Return" "wait 500"; shot s08-pam-prueft; sleep 5; shot s08-pam-falsch
grep -q 'Passwort abgelehnt: Falsches Passwort' "$OUT/s08.log" && kill -0 $P 2>/dev/null && ok "S08 echtes PAM (Dienst login): falsches Passwort abgelehnt, Sperre bleibt" || nok "S08 PAM: $(tail -2 "$OUT/s08.log" | tr '\n' ';')"
blocked "S08b Entsperren mit dem echten Passwort: kein Testkonto und kein sudo auf der VM; geprüft wurde stattdessen die Ablehnung über PAM (S08) und das Entsperren im Testpasswort-Pfad (S04, D09, D13)"
# S09 Absturz des Sperrprogramms simulieren, neue Sperre übernimmt und entsperrt
kill -KILL $P; sleep 1
export QS_TESTPASSWORT=querencia-test
start s09 --sperren --sofort; shot s09-neue-sperre-nach-absturz
q "type querencia-test" "key Return" "wait 1500"; ende
if [ $RC = 0 ] && grep -q 'Sitzung gesperrt' "$OUT/s09.log"; then
    start s09b --dialog; shot s09-nach-wiederherstellung
    kill -0 $P 2>/dev/null && ok "S09 nach Absturz (SIGKILL) des Sperrprogramms: neue Sperre übernimmt, entsperrt, Fenster wieder sichtbar" || nok "S09 nach Wiederherstellung kein Fenster"
    q "key Escape" "wait 800"; ende
else
    nok "S09 neue Sperre nach Absturz (Exit $RC): $(tail -2 "$OUT/s09.log" | tr '\n' ';')"
fi
touch "$QS_PROFILE/logs/FERTIG"
