#!/bin/bash
# org.freedesktop.ScreenSaver des Leerlaufdienstes (QD-104, 0.1.4), nur im
# isolierten labwc mit eigenem Sitzungsbus (dbus-run-session): Name und
# Objekte, Inhibit hält die Leerlaufsperre an, UnInhibit und Prozessende
# geben sie frei, Lock/SetActive/GetActive/ActiveChanged, Sperre vom
# Tastenkürzel, fremdes Cookie, SimulateUserActivity, xdg-desktop-portal-gtk
# über den Dienst. Belege: busctl-/gdbus-Ausgaben in $OUT/schoner-*.txt.
. "$(dirname "$0")/gemeinsam.sh"
: >"$R"
W=$(dirname "$EINGABE")
CLIENT="python3 -I $W/schoner-client.py"
B=$OUT/schoner-busctl.txt
: >"$B"
# Sicherheitsnetz: nur der Wegwerf-Bus, nie der Bus der Sitzung.
case ${DBUS_SESSION_BUS_ADDRESS:-} in
"" | *"/run/user/"*) nok "Sicherheitsnetz: Sitzungsbus $DBUS_SESSION_BUS_ADDRESS ist nicht der Wegwerf-Bus"; touch "$QS_PROFILE/logs/FERTIG"; exit 1 ;;
esac
echo "INFO Bus $DBUS_SESSION_BUS_ADDRESS" >>"$R"
S="busctl --user --timeout=5"
bc() { local aus; aus=$($S "$@" 2>&1); printf '$ busctl --user %s\n%s\n' "$*" "$aus" >>"$B"; printf '%s\n' "$aus"; }
aktiv() { $S call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver GetActive 2>&1; }
gestartet() { grep -c 'sperren --sofort gestartet' "$OUT/s00.log"; }
# Wartet bis $2 Sekunden, bis Datei $1 mindestens $3 Zeilen hat.
zeilen() { local i; for ((i = 0; i < $2 * 10; i++)); do [ "$(wc -l <"$1")" -ge "$3" ] && return 0; sleep 0.1; done; return 1; }
# Wartet bis $2 Sekunden auf Muster $1 im Dienstprotokoll.
im_log() { local i; for ((i = 0; i < $2 * 10; i++)); do grep -q -- "$1" "$OUT/s00.log" && return 0; sleep 0.1; done; return 1; }
entsperren() { q "type querencia-test" "key Return" "wait 1500"; }
zombies() { ps -o pid=,stat= --ppid "$L" 2>/dev/null | awk '$2 ~ /^Z/ {print $1}' | tr '\n' ' '; }

export QS_TESTPASSWORT=querencia-test
"$BIN" --leerlauf-sekunden 8 >"$OUT/s00.log" 2>&1 & L=$!
im_log 'auf dem Sitzungsbus übernommen' 5
gdbus monitor --session --dest org.freedesktop.ScreenSaver >"$OUT/schoner-monitor.txt" 2>&1 & MON=$!
q "move 100 100" "wait 100"

# S01 Name und Besitzer
st=$(bc status org.freedesktop.ScreenSaver)
[ "$(sed -n 's/^PID=//p' <<<"$st")" = "$L" ] && ok "S01 org.freedesktop.ScreenSaver gehört dem Leerlaufdienst (PID $L)" || nok "S01 $(tr '\n' ';' <<<"$st" | cut -c1-200)"
# S02 Schnittstelle an beiden Pfaden
n=0
for pfad in /org/freedesktop/ScreenSaver /ScreenSaver; do
    i=$(bc introspect org.freedesktop.ScreenSaver "$pfad" org.freedesktop.ScreenSaver)
    for m in .Inhibit .UnInhibit .Lock .GetActive .GetActiveTime .SetActive .SimulateUserActivity .ActiveChanged; do
        grep -q "^\\$m " <<<"$i" && n=$((n + 1))
    done
done
[ $n = 16 ] && ok "S02 Schnittstelle an /org/freedesktop/ScreenSaver und /ScreenSaver (7 Methoden, 1 Signal je Pfad)" || nok "S02 nur $n/16 Einträge"
# S03 GetActive ohne Sperre
a=$(bc call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver GetActive)
[ "$a" = "b false" ] && ok "S03 GetActive ohne Sperre: $a" || nok "S03 GetActive: $a"

# S04 Inhibit hält die Leerlaufsperre an (Client hält die Verbindung)
mkfifo "$QS_PROFILE/c1.in"
$CLIENT <"$QS_PROFILE/c1.in" >"$OUT/schoner-c1.txt" 2>&1 & C1=$!
exec 7>"$QS_PROFILE/c1.in"
echo "name" >&7; echo "inhibit Firefox video-playing /ScreenSaver" >&7
zeilen "$OUT/schoner-c1.txt" 5 2
C1NAME=$(sed -n 1p "$OUT/schoner-c1.txt"); COOKIE=$(sed -n 's/^cookie //p' "$OUT/schoner-c1.txt")
q "move 110 110" "wait 100"; sleep 11
shot s04-gehemmt
[ -n "$COOKIE" ] && grep -q 'Sperre gehemmt durch 1 Hemmung(en): Firefox (video-playing)' "$OUT/s00.log" && [ "$(gestartet)" = 0 ] &&
    ok "S04 Inhibit von $C1NAME (Cookie $COOKIE, /ScreenSaver): Leerlauf nach 8 s erreicht, keine Sperre (Bild s04)" || nok "S04 Cookie=$COOKIE gestartet=$(gestartet) $(tail -3 "$OUT/s00.log" | tr '\n' ';')"
# S05 UnInhibit: Leerlauf zählt von vorn, Sperre nach 6 s
echo "uninhibit $COOKIE /ScreenSaver" >&7
zeilen "$OUT/schoner-c1.txt" 5 3
im_log 'Leerlauf zählt von vorn (letzte Hemmung aufgehoben)' 3 && t0=$(date +%s.%N)
sleep 3
[ "$(gestartet)" = 0 ] && f1=ja || f1=nein
im_log 'Leerlauf: .*gestartet (PID [0-9]*, Nr. 1)' 7 && t1=$(date +%s.%N)
d=$(python3 -c "print(round(${t1:-0}-${t0:-0},1))")
[ "$(sed -n 3p "$OUT/schoner-c1.txt")" = ok ] && [ $f1 = ja ] && [ "$(gestartet)" = 1 ] &&
    ok "S05 UnInhibit $COOKIE: Leerlauf zählt von vorn, nach 3 s noch keine Sperre, Sperre Nr. 1 nach ${d} s" || nok "S05 f1=$f1 gestartet=$(gestartet) d=$d"
sleep 1
# S06 GetActive/GetActiveTime gesperrt, ActiveChanged(true)
a=$(bc call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver GetActive)
sleep 1.2
at=$(bc call org.freedesktop.ScreenSaver /ScreenSaver org.freedesktop.ScreenSaver GetActiveTime)
shot s06-gesperrt
[ "$a" = "b true" ] && [ "${at#u }" -ge 1 ] && grep -q 'ActiveChanged (true,)' "$OUT/schoner-monitor.txt" &&
    ok "S06 gesperrt: GetActive $a, GetActiveTime $at, Signal ActiveChanged(true) (Bild s06)" || nok "S06 $a / $at / $(grep -c ActiveChanged "$OUT/schoner-monitor.txt")"
# S07 Entsperren: ActiveChanged(false) gleich nach dem Ernten
entsperren
a=$(aktiv)
im_log 'ActiveChanged(false)' 3
[ "$a" = "b false" ] && grep -q 'ActiveChanged (false,)' "$OUT/schoner-monitor.txt" && grep -q 'geerntet' "$OUT/s00.log" &&
    ok "S07 entsperrt: GetActive $a, Signal ActiveChanged(false), Sperrprozess geerntet" || nok "S07 $a / $(grep ActiveChanged "$OUT/schoner-monitor.txt" | tr '\n' ';')"

# S08 Prozessende räumt auf: Client 2 hemmt und wird mit SIGKILL beendet
mkfifo "$QS_PROFILE/c2.in"
$CLIENT <"$QS_PROFILE/c2.in" >"$OUT/schoner-c2.txt" 2>&1 & C2=$!
exec 8>"$QS_PROFILE/c2.in"
echo "name" >&8; echo "inhibit mpv Wiedergabe" >&8; echo "inhibit mpv Wiedergabe" >&8
zeilen "$OUT/schoner-c2.txt" 5 3
C2NAME=$(sed -n 1p "$OUT/schoner-c2.txt")
q "move 120 120" "wait 100"; sleep 9.5
[ "$(gestartet)" = 1 ] && f1=ja || f1=nein
kill -KILL $C2; wait $C2 2>/dev/null; exec 8>&-
im_log "$C2NAME hat den Bus verlassen" 3 && t0=$(date +%s.%N)
im_log 'Leerlauf: .*gestartet (PID [0-9]*, Nr. 2)' 10 && t1=$(date +%s.%N)
d=$(python3 -c "print(round(${t1:-0}-${t0:-0},1))")
grep -q "$C2NAME hat den Bus verlassen, Cookie(s) \[[0-9]*, [0-9]*\] aufgehoben, 0 gelten noch" "$OUT/s00.log" && [ $f1 = ja ] && [ "$(gestartet)" = 2 ] &&
    ok "S08 zwei Hemmungen von $C2NAME, SIGKILL: beide aufgehoben, vorher keine Sperre, Sperre Nr. 2 nach ${d} s" || nok "S08 f1=$f1 gestartet=$(gestartet) $(grep 'verlassen' "$OUT/s00.log")"
entsperren

# S09 Lock, zweites Lock, SetActive(false)
bc call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver Lock >/dev/null
sleep 1.5
a=$(aktiv)
bc call org.freedesktop.ScreenSaver /ScreenSaver org.freedesktop.ScreenSaver Lock >/dev/null
sf=$(bc call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver SetActive b false)
shot s09-lock
[ "$a" = "b true" ] && grep -q 'Lock: .*gestartet (PID [0-9]*, Nr. 3)' "$OUT/s00.log" && grep -q 'Lock: Sperrbildschirm läuft bereits' "$OUT/s00.log" && [ "$sf" = "b false" ] && [ "$(gestartet)" = 3 ] &&
    ok "S09 Lock sperrt (Nr. 3, GetActive $a), zweites Lock startet keine zweite Sperre, SetActive(false) → $sf (Bild s09)" || nok "S09 $a / $sf / gestartet=$(gestartet)"
entsperren
# S10 SetActive(true) sperrt
st=$(bc call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver SetActive b true)
sleep 1.5
a=$(aktiv)
[ "$st" = "b true" ] && [ "$a" = "b true" ] && grep -q 'SetActive: .*gestartet (PID [0-9]*, Nr. 4)' "$OUT/s00.log" &&
    ok "S10 SetActive(true) → $st, Sperre Nr. 4, GetActive $a" || nok "S10 $st / $a"
entsperren

# S11 Sperre vom Tastenkürzel (eigener Prozess, nicht vom Dienst gestartet)
"$BIN" --sperren --sofort >"$OUT/s11.log" 2>&1 & K=$!
sleep 6
a=$(aktiv)
n_true=$(grep -c 'ActiveChanged (true,)' "$OUT/schoner-monitor.txt")
entsperren; wait $K; rk=$?
q "move 125 125" "wait 100"
for ((i = 0; i < 60; i++)); do a2=$(aktiv); [ "$a2" = "b false" ] && break; sleep 0.1; done
[ "$a" = "b true" ] && [ "$n_true" -ge 5 ] && [ "$a2" = "b false" ] && [ $rk = 0 ] && [ "$(gestartet)" = 4 ] &&
    ok "S11 Sperre vom Tastenkürzel (PID $K): GetActive $a, ActiveChanged(true) gemeldet, nach dem Entsperren $a2" || nok "S11 $a / true×$n_true / $a2 / Exit $rk"

# S12 Fremdes Cookie bleibt; danach Client 1 ordentlich beenden
echo "inhibit Chromium Video" >&7
zeilen "$OUT/schoner-c1.txt" 5 4
COOKIE=$(sed -n '4s/^cookie //p' "$OUT/schoner-c1.txt")
bc call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver UnInhibit u "$COOKIE" >/dev/null
im_log "UnInhibit $COOKIE von .*: gehört einem anderen Absender, bleibt" 3 && f1=ja || f1=nein
q "move 130 130" "wait 100"; sleep 9.5
[ "$(gestartet)" = 4 ] && f2=ja || f2=nein
echo "ende" >&7; exec 7>&-; wait $C1
im_log "$C1NAME hat den Bus verlassen, Cookie(s) \[$COOKIE\]" 3 && f3=ja || f3=nein
[ $f1 = ja ] && [ $f2 = ja ] && [ $f3 = ja ] &&
    ok "S12 UnInhibit eines fremden Cookies ($COOKIE) über busctl: bleibt, keine Sperre; Client endet: Cookie aufgehoben" || nok "S12 f1=$f1 f2=$f2 f3=$f3"
im_log 'Leerlauf: .*gestartet (PID [0-9]*, Nr. 5)' 10 && entsperren

# S13 SimulateUserActivity zählt den Leerlauf von vorn
q "move 140 140" "wait 100"; sleep 5
bc call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver SimulateUserActivity >/dev/null
sleep 5
n=$(gestartet)
im_log 'Leerlauf: .*gestartet (PID [0-9]*, Nr. 6)' 6 && f1=ja || f1=nein
[ "$n" = 5 ] && [ $f1 = ja ] && grep -q 'Leerlauf zählt von vorn (SimulateUserActivity)' "$OUT/s00.log" &&
    ok "S13 SimulateUserActivity nach 5 s: 10 s nach der letzten Eingabe noch keine Sperre, danach Sperre Nr. 6" || nok "S13 n=$n f1=$f1"
entsperren

# S14 xdg-desktop-portal-gtk hemmt über den Dienst (Inhibit-Backend, Flag 8 = Leerlauf)
PG=/usr/libexec/xdg-desktop-portal-gtk
if [ -x "$PG" ]; then
    G_MESSAGES_DEBUG=all "$PG" -v >"$OUT/schoner-portal.log" 2>&1 & PGP=$!
    for ((i = 0; i < 50; i++)); do $S status org.freedesktop.impl.portal.desktop.gtk >/dev/null 2>&1 && break; sleep 0.1; done
    H=/org/freedesktop/portal/desktop/request/1_99/qd104
    echo "\$ gdbus call … org.freedesktop.impl.portal.Inhibit.Inhibit $H org.example.Video '' 8 {}" >>"$B"
    gdbus call --session --timeout 10 --dest org.freedesktop.impl.portal.desktop.gtk --object-path /org/freedesktop/portal/desktop \
        --method org.freedesktop.impl.portal.Inhibit.Inhibit "$H" org.example.Video "" 8 "{}" >>"$B" 2>&1
    im_log 'Inhibit von .*→ Cookie' 3
    q "move 150 150" "wait 100"; sleep 9.5
    [ "$(gestartet)" = 6 ] && f1=ja || f1=nein
    echo "\$ gdbus call … org.freedesktop.impl.portal.Request.Close ($H)" >>"$B"
    gdbus call --session --timeout 10 --dest org.freedesktop.impl.portal.desktop.gtk --object-path "$H" \
        --method org.freedesktop.impl.portal.Request.Close >>"$B" 2>&1
    im_log 'Leerlauf zählt von vorn (letzte Hemmung aufgehoben)' 3
    pi=$(grep -c 'Inhibit von .*→ Cookie' "$OUT/s00.log")
    kill -TERM $PGP; wait $PGP 2>/dev/null
    grep -q 'Using org.freedesktop.ScreenSaver for inhibit' "$OUT/schoner-portal.log" && ! grep -q 'Backend call failed' "$OUT/schoner-portal.log" && [ $f1 = ja ] &&
        grep -q 'UnInhibit [0-9]* von .*, 0 gelten noch' "$OUT/s00.log" &&
        ok "S14 xdg-desktop-portal-gtk: „Using org.freedesktop.ScreenSaver for inhibit“, Inhibit hält die Sperre an, Close hebt auf, kein „Backend call failed“ ($pi Inhibit gesamt)" ||
        nok "S14 f1=$f1 $(grep -E 'ScreenSaver|failed' "$OUT/schoner-portal.log" | tr '\n' ';' | cut -c1-300)"
    im_log 'Leerlauf: .*gestartet (PID [0-9]*, Nr. 7)' 10 && entsperren
else
    blocked "S14 $PG fehlt"
fi

# S15 keine Zombie-Kinder, Dienst lebt
sleep 1
[ -z "$(zombies)" ] && kill -0 $L && ok "S15 nach $(gestartet) Sperren kein Zombie-Kind, Leerlaufdienst läuft" || nok "S15 Zombies: $(zombies)"
kill -TERM $MON $L; wait $L 2>/dev/null
cp "$OUT/s00.log" "$OUT/schoner-dienst.log"
echo "Ergebnis ScreenSaver: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$QS_PROFILE/logs/FERTIG"
