#!/bin/bash
# polkit-Agent nach einem Neustart von polkitd (0.1.8, QD-156): der Agent
# beobachtet NameOwnerChanged für org.freedesktop.PolicyKit1 und registriert
# sich beim neuen Besitzer erneut (gleiches Subjekt, gleiche Locale), mit
# Wiederholung, solange der neue polkitd nicht antwortet. Alles läuft an
# einem eigenen Wegwerf-Systembus mit polkitd-attrappe.py; das echte polkitd
# und die echte logind-Sitzung werden nie gefragt (Subjekt aus
# XDG_SESSION_ID=q1). Die Prüfanfragen nennen das gesperrte Systemkonto
# nobody (uid 65534) und werden mit Esc abgebrochen, bevor ein Passwort
# eingegeben ist: kein echtes Konto, keine Anmeldung. Der Helfer
# polkit-agent-helper-1 fragt dabei nur nach dem Passwort.
#   N01 erste Registrierung      N05 Neustart, neuer polkitd verweigert 2×
#   N02 Anfrage vorher           N06 Anfrage danach
#   N03 Name abgeben und wieder  N07 offene Anfrage beim Neustart verworfen,
#       nehmen (derselbe Prozess)    neuer polkitd schweigt 1× (Zeitgrenze)
#   N04 Anfrage danach           N08 Anfrage danach
#   N09 ein Agent-Prozess, Subjekt und Locale überall gleich
. "$(dirname "$0")/gemeinsam.sh"
W=$(dirname "$EINGABE")
[ -x "$AGENT" ] || { nok "N00 Agent fehlt: $AGENT"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }

# Wegwerf-Systembus
SYSBUS=$XDG_RUNTIME_DIR/systembus-ersatz
PL=$OUT/n-polkitd.log
AL=$OUT/n-agent.log
: >"$PL"
dbus-daemon --session --nofork --nopidfile --address="unix:path=$SYSBUS" >>"$OUT/n-systembus.log" 2>&1 &
BUSPID=$!
for ((i = 0; i < 50; i++)); do [ -S "$SYSBUS" ] && break; sleep 0.1; done
export DBUS_SYSTEM_BUS_ADDRESS="unix:path=$SYSBUS"
[[ $DBUS_SYSTEM_BUS_ADDRESS == unix:path=*/qed-runtime-*/systembus-ersatz ]] ||
    { nok "Sicherheitsnetz: DBUS_SYSTEM_BUS_ADDRESS $DBUS_SYSTEM_BUS_ADDRESS"; kill $BUSPID; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
export XDG_SESSION_ID=q1
ATTR=
aufraeumen() { kill $ATTR $AG $BUSPID 2>/dev/null; wait $ATTR $AG $BUSPID 2>/dev/null; }
# Startet eine Attrappe mit den Optionen "$@"; PID in ATTR.
attrappe() {
    rm -f "$QS_PROFILE/logs/polkitd-bereit"
    python3 -I "$W/polkitd-attrappe.py" "$DBUS_SYSTEM_BUS_ADDRESS" "$PL" "$QS_PROFILE/logs/polkitd-bereit" "$@" 2>>"$OUT/n-systembus.log" &
    ATTR=$!
    for ((i = 0; i < 50; i++)); do [ -f "$QS_PROFILE/logs/polkitd-bereit" ] && return 0; sleep 0.1; done
    nok "Attrappe startet nicht ($*)"; aufraeumen; touch "$QS_PROFILE/logs/FERTIG"; exit 1
}
# Wartet bis zu $2 s, bis Datei $1 die Zeile $3 (grep -E) $4-mal (Vorgabe 1) enthält.
warte() {
    local n=${4:-1} t
    for ((t = 0; t < $2 * 10; t++)); do
        [ "$(grep -cE "$3" "$1")" -ge "$n" ] && return 0
        sleep 0.1
    done
    return 1
}
# Anfrage über Attrappe $1 (PID): Dialog muss erscheinen, Esc bricht ab,
# die Attrappe erhält Cancelled. $2 Fall, $3 Bildname.
anfrage_pruefen() {
    local vorher_a vorher_c vorher_h
    vorher_a=$(grep -c 'Anfrage: org.querencia.attrappe.pruefen' "$AL")
    vorher_c=$(grep -c 'ANTWORT .* Fehler org.freedesktop.PolicyKit1.Error.Cancelled' "$PL")
    vorher_h=$(grep -c 'Helfer fragt' "$AL")
    kill -USR1 "$1"
    if warte "$AL" 5 'Anfrage: org.querencia.attrappe.pruefen' $((vorher_a + 1)); then
        # Esc erst, wenn der Helfer fragt (Eingabefeld aktiv, Fenster fertig)
        warte "$AL" 5 'Helfer fragt' $((vorher_h + 1))
        sleep 0.7; shot "$3"
        q "key Escape" "wait 500"
        if warte "$PL" 5 'ANTWORT .* Fehler org.freedesktop.PolicyKit1.Error.Cancelled' $((vorher_c + 1)); then
            ok "$2 Anfrage der Attrappe erreicht den Agenten: Dialog ($3.png), Esc, Antwort Cancelled ($(grep 'ANTWORT' "$PL" | tail -1 | cut -d' ' -f3-5))"
        else
            nok "$2 keine Cancelled-Antwort: $(tail -2 "$PL" | tr '\n' ';') / $(tail -3 "$AL" | tr '\n' ';')"
        fi
    else
        nok "$2 Anfrage kommt nicht an: $(tail -2 "$PL" | tr '\n' ';') / $(tail -3 "$AL" | tr '\n' ';')"
    fi
}

# N01 erste Registrierung
attrappe
A1=$ATTR
"$AGENT" >"$AL" 2>&1 & AG=$!
sleep 0.5
tr '\0' '\n' <"/proc/$AG/environ" | grep -qx "DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SYSTEM_BUS_ADDRESS" ||
    { nok "Sicherheitsnetz: Agent ohne Wegwerf-Systembus"; aufraeumen; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
if warte "$AL" 5 'registriert bei polkitd' && warte "$PL" 5 'REGISTRIERT #1'; then
    ok "N01 erste Registrierung: $(grep -o 'registriert bei polkitd.*' "$AL" | head -1); Attrappe: $(grep -o 'REGISTRIERT #1.*' "$PL" | cut -d' ' -f1-5)"
else
    nok "N01 $(tr '\n' ';' <"$AL")"
fi
# N02 Anfrage vor dem Neustart
anfrage_pruefen $A1 N02 n02-vorher

# N03 derselbe Prozess gibt den Namen ab und nimmt ihn wieder
kill -USR2 $A1
warte "$AL" 5 'polkitd beendet \(Besitzer' && ok "N03a Name abgegeben: Agent meldet „$(grep -o 'polkitd beendet (Besitzer.*' "$AL" | tail -1)“" || nok "N03a $(tail -3 "$AL" | tr '\n' ';')"
sleep 1
kill -USR2 $A1
if warte "$AL" 5 'polkitd neu gestartet, erneut registriert' && warte "$PL" 5 'REGISTRIERT #2'; then
    ok "N03b Name wieder genommen: „$(grep -o 'polkitd neu gestartet, erneut registriert.*' "$AL" | tail -1)“"
else
    nok "N03b $(tail -4 "$AL" | tr '\n' ';')"
fi
anfrage_pruefen $A1 N04 n04-nach-rueckgabe

# N05 polkitd endet, ein neuer verweigert die ersten beiden Registrierungen
kill -TERM $A1; wait $A1 2>/dev/null
warte "$AL" 5 'polkitd beendet \(Besitzer' 2 && ok "N05a polkitd beendet: „$(grep -o 'polkitd beendet (Besitzer.*' "$AL" | tail -1)“" || nok "N05a $(tail -3 "$AL" | tr '\n' ';')"
sleep 1.5
attrappe --verweigern 2
A2=$ATTR
if warte "$AL" 10 'polkitd neu gestartet, erneut registriert' 2; then
    zeilen=$(grep -c 'antwortet noch nicht' "$AL")
    grep -q 'erneut registriert (polkitd [^,]*, Versuch 3,' "$AL" && [ "$zeilen" = 2 ] &&
        ok "N05b neuer polkitd verweigert 2×: Wiederholung nach 500 und 1000 ms, „$(grep -o 'polkitd neu gestartet, erneut registriert.*' "$AL" | tail -1)“" ||
        nok "N05b Versuche: $(grep -E 'antwortet noch nicht|erneut registriert' "$AL" | tail -3 | tr '\n' ';')"
    # Abstände laut Attrappe (ms, monotone Uhr)
    t=$(grep -E "^[0-9]+ \\[$A2\\] (VERWEIGERT|REGISTRIERT) #" "$PL" | cut -d' ' -f1 | tr '\n' ' ')
    set -- $t
    if [ $# = 3 ]; then
        d1=$(($2 - $1)) d2=$(($3 - $2))
        [ $d1 -ge 450 ] && [ $d1 -le 900 ] && [ $d2 -ge 950 ] && [ $d2 -le 1500 ] &&
            ok "N05c Wartezeiten gemessen: $d1 ms und $d2 ms (Soll 500 und 1000 ms)" || nok "N05c Wartezeiten $d1 ms, $d2 ms"
    else
        nok "N05c Zeitstempel: $t"
    fi
else
    nok "N05b $(tail -5 "$AL" | tr '\n' ';')"
fi
anfrage_pruefen $A2 N06 n06-nach-neustart

# N07 offene Anfrage beim Neustart; der nächste polkitd schweigt einmal
kill -USR1 $A2
if warte "$AL" 5 'Anfrage: org.querencia.attrappe.pruefen' 4; then
    sleep 1.5; shot n07-offen
    kill -TERM $A2; wait $A2 2>/dev/null
    if warte "$AL" 5 'abgebrochen: polkitd beendet'; then
        sleep 0.5; shot n07-verworfen
        ok "N07a offene Anfrage beim Ende von polkitd verworfen: „$(grep -o 'abgebrochen: polkitd beendet.*' "$AL" | tail -1)“, Dialog zu (n07-verworfen.png)"
    else
        nok "N07a $(tail -3 "$AL" | tr '\n' ';')"
    fi
else
    nok "N07a Anfrage kommt nicht an"
fi
attrappe --schweigen 1
A3=$ATTR
if warte "$AL" 15 'polkitd neu gestartet, erneut registriert' 3; then
    grep -q 'erneut registriert (polkitd [^,]*, Versuch 2,' "$AL" && grep -qE 'antwortet noch nicht \(Versuch 1: .*(Timeout|Zeit|NoReply)' "$AL" &&
        ok "N07b neuer polkitd schweigt: Zeitgrenze 5 s, Wiederholung, „$(grep -o 'polkitd neu gestartet, erneut registriert.*' "$AL" | tail -1)“" ||
        nok "N07b $(grep -E 'antwortet noch nicht|erneut registriert' "$AL" | tail -2 | tr '\n' ';')"
else
    nok "N07b $(tail -4 "$AL" | tr '\n' ';')"
fi
anfrage_pruefen $A3 N08 n08-nach-zeitgrenze

# N09 derselbe Agent-Prozess, überall dasselbe Subjekt und dieselbe Locale
paare=$(grep -o 'REGISTRIERT #[0-9]* [^ ]* subjekt=[^ ]* locale=[^ ]* objekt=[^ ]*' "$PL" | cut -d' ' -f4- | sort -u)
anzahl=$(grep -c 'REGISTRIERT #' "$PL")
# Locale wie bei der ersten Registrierung (aus LANGUAGE, de: de_DE.UTF-8)
erste=$(grep -o 'registriert bei polkitd .*Locale [^,]*' "$AL" | head -1 | sed 's/.*Locale //')
if kill -0 $AG 2>/dev/null && [ "$(echo "$paare" | wc -l)" = 1 ] && [ "$anzahl" = 4 ] &&
    [ "$paare" = "subjekt=unix-session:session-id=q1 locale=$erste objekt=/org/querencia/PolkitAgent" ]; then
    ok "N09 Agent $AG läuft durchgehend, 4 Registrierungen mit gleichem Subjekt und gleicher Locale ($paare)"
else
    nok "N09 Agent lebt: $(kill -0 $AG 2>/dev/null && echo ja || echo nein), $anzahl Registrierungen: $(echo "$paare" | tr '\n' ';')"
fi
kill -TERM $AG; wait $AG 2>/dev/null
aufraeumen
touch "$QS_PROFILE/logs/FERTIG"
