#!/bin/bash
# polkit-Agent: ohne Agent, Registrierung, Dialog, falsches Passwort, Abbrechen
# (Esc, Schaltfläche), Rückzug durch polkitd. Der Agent registriert sich für
# die logind-Sitzung dieses Prozesses (die echte Sitzung 10), nur für die
# Dauer des Szenarios; seine Fenster erscheinen im isolierten labwc.
. "$(dirname "$0")/gemeinsam.sh"
[ -x "$AGENT" ] || { nok "P00 Agent fehlt: $AGENT"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
# Die Prüfprozesse laufen außerhalb jeder logind-Sitzung (App-Scope der
# Werkstatt-Sitzung). Der Agent registriert sich darum ausdrücklich für die
# Sitzung des echten labwc (XDG_SESSION_ID), und pkcheck fragt für dessen
# PID; nur Lesezugriff auf die echte Sitzung, ihr Bildschirm bleibt unberührt.
LIVE_LABWC=$(for p in $(pgrep -u "$(id -u)" -x labwc); do tr '\0' '\n' </proc/$p/environ 2>/dev/null | grep -q '^XDG_RUNTIME_DIR=/run/user/' && echo $p && break; done)
[ -n "$LIVE_LABWC" ] || { nok "P00 kein labwc der echten Sitzung gefunden"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
SITZUNG=$(busctl --system call org.freedesktop.login1 /org/freedesktop/login1 org.freedesktop.login1.Manager GetSessionByPID u "$LIVE_LABWC" | sed -n 's|.*/session/_3||p' | tr -d '"')
[ -n "$SITZUNG" ] || { nok "P00 Sitzung des labwc $LIVE_LABWC nicht ermittelbar"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
echo "Sitzung $SITZUNG, labwc $LIVE_LABWC" >"$OUT/p00-sitzung.txt"
export XDG_SESSION_ID=$SITZUNG
PK_PROZESS=$LIVE_LABWC
# P01 ohne Agent: Neustart/Herunterfahren für die aktive Sitzung ohne Legitimierung erlaubt
pkcheck --action-id org.freedesktop.login1.reboot --process $PK_PROZESS -u >"$OUT/p01-reboot.txt" 2>&1; r1=$?
pkcheck --action-id org.freedesktop.login1.power-off --process $PK_PROZESS -u >"$OUT/p01-poweroff.txt" 2>&1; r2=$?
[ $r1 = 0 ] && [ $r2 = 0 ] && ok "P01 ohne Agent: login1.reboot und login1.power-off für die aktive Sitzung erlaubt (pkcheck Exit 0, implicit active = yes)" || nok "P01 pkcheck reboot=$r1 poweroff=$r2"
# P02 ohne Agent: auth_admin-Aktion scheitert mit „no agent“
timeout 10 pkcheck --action-id org.freedesktop.policykit.exec --process $PK_PROZESS -u >"$OUT/p02.txt" 2>&1; rc=$?
[ $rc != 0 ] && grep -qi 'no agent' "$OUT/p02.txt" && ok "P02 ohne Agent: policykit.exec nicht legitimierbar („no agent is available“, Exit $rc)" || nok "P02 (Exit $rc): $(tr '\n' ';' <"$OUT/p02.txt")"
# P03 Agent registrieren
"$AGENT" >"$OUT/p03-agent.log" 2>&1 & AG=$!
sleep 2.5
grep -q 'registriert bei polkitd' "$OUT/p03-agent.log" && kill -0 $AG 2>/dev/null && ok "P03 Agent registriert ($(grep -o 'Subjekt [^,]*, Locale [^,]*' "$OUT/p03-agent.log"))" || nok "P03 Agent: $(tr '\n' ';' <"$OUT/p03-agent.log")"
# P04 Dialog erscheint
timeout 60 pkcheck --action-id org.freedesktop.policykit.exec --process $PK_PROZESS -u >"$OUT/p04.txt" 2>&1 & PK=$!
sleep 2.5; shot p04-legitimierung
$MITTE "$OUT/p04-legitimierung.png" >"$OUT/p04-mitte.txt" && grep -q 'Anfrage: org.freedesktop.policykit.exec' "$OUT/p03-agent.log" && ok "P04 pkcheck -u: Legitimierungsdialog erscheint mittig ($(cat "$OUT/p04-mitte.txt" | grep -o 'Fenster [^,]*'))" || nok "P04 Dialog: $(cat "$OUT/p04-mitte.txt")"
grep -q 'Helfer fragt' "$OUT/p03-agent.log" && ok "P04b Helfer polkit-agent-helper-1 fragt nach dem Passwort ($(grep -o 'Helfer fragt: [^(]*' "$OUT/p03-agent.log" | head -1))" || nok "P04b kein Helfer-Prompt"
# P05 falsches Passwort
q "move 640 600" "type falsch" "key Return" "wait 500"; shot p05-prueft; sleep 4; shot p05-falsch
grep -q 'fehlgeschlagen, Versuch 1' "$OUT/p03-agent.log" && kill -0 $PK 2>/dev/null && ok "P05 falsches Passwort: Helfer meldet Fehlschlag, Dialog bleibt für Versuch 2" || nok "P05: $(tail -3 "$OUT/p03-agent.log" | tr '\n' ';')"
# P06 Esc bricht ab
sleep 1; q "key Escape" "wait 1500"; wait $PK; rc=$?
[ $rc != 0 ] && grep -q 'abgebrochen: Esc' "$OUT/p03-agent.log" && ok "P06 Esc: Anfrage abgebrochen, pkcheck Exit $rc ($(tr '\n' ' ' <"$OUT/p04.txt" | cut -c1-80))" || nok "P06 Esc (Exit $rc): $(tr '\n' ';' <"$OUT/p04.txt")"
# P07 Schaltfläche Abbrechen (Alt+A); pkexec ist auf der VM nicht installiert
timeout 60 pkcheck --action-id org.freedesktop.policykit.exec --process $PK_PROZESS -u >"$OUT/p07.txt" 2>&1 & PX=$!
sleep 2.5; shot p07-abbrechen-knopf
q "chord alt a" "wait 1500"; wait $PX; rc=$?
[ $rc != 0 ] && grep -q 'abgebrochen: Schaltfläche' "$OUT/p03-agent.log" && ok "P07 Alt+A (Schaltfläche Abbrechen): Anfrage abgebrochen, pkcheck Exit $rc" || nok "P07 Exit $rc: $(tr '\n' ';' <"$OUT/p07.txt")"
# P08 zweite Anfrage, während eine offen ist: sofort abgewiesen, erste bleibt
timeout 60 pkcheck --action-id org.freedesktop.policykit.exec --process $PK_PROZESS -u >"$OUT/p08.txt" 2>&1 & PK=$!
sleep 2.5
timeout 20 pkcheck --action-id org.freedesktop.policykit.exec --process $PK_PROZESS -u >"$OUT/p08b.txt" 2>&1; rc2=$?
shot p08-erste-anfrage-bleibt
grep -q 'abgewiesen: es ist schon eine offen' "$OUT/p03-agent.log" && [ $rc2 != 0 ] && kill -0 $PK 2>/dev/null && ok "P08 zweite Anfrage während einer offenen: abgewiesen (Exit $rc2), erste bleibt" || nok "P08 (zweite Exit $rc2): $(tail -2 "$OUT/p03-agent.log" | tr '\n' ';')"
q "key Escape" "wait 1500"; wait $PK
blocked "P09 erfolgreiche Legitimierung mit echtem Passwort: kein Testkonto, kein sudo; geprüft sind Registrierung, Helfer-Prompt, Ablehnung, Abbruch (Esc, Schaltfläche) und Abweisung paralleler Anfragen; CancelAuthentication von polkitd nicht auslösbar (pkcheck zieht bei SIGINT nicht zurück)"
kill -TERM $AG; wait $AG 2>/dev/null
touch "$QS_PROFILE/logs/FERTIG"
