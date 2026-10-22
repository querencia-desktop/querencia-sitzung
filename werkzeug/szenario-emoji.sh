#!/bin/bash
# labwc-Titelleiste mit Emoji (QD-123): je Testtitel ein foot-Fenster mit
# Serverdekoration, Foto der Titelleiste. rc.xml: die Vorgabe der Sitzung
# (gui-pruefung.sh mit QS_RC_XML), Locale wie live (QS_LANG=C.UTF-8).
. "$(dirname "$0")/gemeinsam.sh"
: >"$R"
W=$(dirname "$EINGABE")
echo "INFO LANG=$LANG labwc $(readlink /proc/$LABWC_PID/exe)" >>"$R"
grep -A3 '<font place="ActiveWindow">' "$XDG_CONFIG_HOME/labwc/rc.xml" | tr -s ' \n' ' ' | sed 's/^/INFO rc.xml /' >>"$R"; echo >>"$R"
echo "INFO fc-match: $(fc-match 'DejaVu Sans:bold') / emoji: $(fc-match emoji)" >>"$R"
# Rohtitel nur im Wegwerfprofil, im Nachweis als Escape
python3 -I "$W/emoji-titel.py" >"$QS_PROFILE/emoji-titel.txt"
python3 -I "$W/emoji-titel.py" --liste >"$OUT/emoji-titel.txt"
while IFS=$'\t' read -r name titel; do
    foot -o csd.preferred=server -W 70x4 -T "$titel" sleep 20 >/dev/null 2>&1 &
    F=$!
    sleep 1.8
    shot "emoji-$name"
    kill $F 2>/dev/null; wait $F 2>/dev/null
    ok "$name Fenster gezeigt (Bild emoji-$name)"
done <"$QS_PROFILE/emoji-titel.txt"
touch "$QS_PROFILE/logs/FERTIG"
