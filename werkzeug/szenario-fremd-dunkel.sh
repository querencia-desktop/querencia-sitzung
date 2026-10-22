#!/bin/bash
# Dunkelmodus für fremde GTK-3-Programme (0.1.5, QD-103). Der
# Leerlaufdienst (`--leerlauf 600`) läuft im isolierten labwc; das
# Farbschema kommt wie live über gsettings und das Settings-Portal
# (portal-ersatz.py, gui-pruefung.sh setzt QS_PORTAL=1). Fremde Zeilen in
# settings.ini müssen byte-gleich bleiben; ein eigener
# gtk-application-prefer-dark-theme des Benutzers gilt (F15). Die
# labwc-Rahmen schaltet die Sitzung nicht (Entscheidung der Startsession):
# themerc-override bleibt unberührt, labwc bekommt kein SIGHUP, die
# Titelleiste bleibt hell.
# Fotos: werkzeug/gtk3-fenster.py (grundfarbe.py) und ein foot-Fenster mit
# Serverdekoration (rahmenfarbe.py).
. "$(dirname "$0")/gemeinsam.sh"
: >"$R"
case ${DBUS_SESSION_BUS_ADDRESS:-} in *"/run/user/"*) nok "Sicherheitsnetz: Sitzungsbus der echten Sitzung"; touch "$QS_PROFILE/logs/FERTIG"; exit 1 ;; esac
W=$(dirname "$EINGABE")
GF="python3 -I $W/grundfarbe.py"
RF="python3 -I $W/rahmenfarbe.py"
INI=$XDG_CONFIG_HOME/gtk-3.0/settings.ini
TRC=$XDG_CONFIG_HOME/labwc/themerc-override
DLOG=$OUT/fd-dienst.log
ANFANG='# >>> querencia-sitzung: Dunkelmodus'
mkdir -p "$(dirname "$INI")"
gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch
gsettings set org.gnome.desktop.interface color-scheme default
thema=$(readlink -f "$XDG_DATA_HOME/themes/Querencia-Klassisch")
echo "INFO Thema $thema, labwc $(readlink /proc/$LABWC_PID/exe) (PID $LABWC_PID)" >>"$R"
# Thema ohne Dunkelfassung (F08): gtk.css ohne gtk-dark.css
for g in gtk-3.0 gtk-4.0; do mkdir -p "$XDG_DATA_HOME/themes/Ohne-Dunkel/$g"; : >"$XDG_DATA_HOME/themes/Ohne-Dunkel/$g/gtk.css"; done
python3 -I "$W/portal-ersatz.py" "$QS_PROFILE/logs/portal-bereit" >>"$OUT/fd-portal.log" 2>&1 &
for ((i = 0; i < 50; i++)); do [ -f "$QS_PROFILE/logs/portal-bereit" ] && break; sleep 0.1; done

# fremde Dateien mit eigenen Zeilen
printf '[Settings]\ngtk-font-name=DejaVu Sans 11\n\n[Andere]\nx=1\n' >"$INI"
printf '# eigene Zeile des Benutzers\nwindow.button.spacing: 2\n' >"$TRC"
cp "$INI" "$QS_PROFILE/ini-fremd"; cp "$TRC" "$QS_PROFILE/trc-fremd"
labwc_stand() { stat -c %Y "$TRC"; ls "$XDG_CONFIG_HOME/labwc"; }
labwc_stand >"$QS_PROFILE/labwc-vorher"

dienst_start() { "$BIN" --leerlauf 600 >>"$DLOG" 2>&1 & D=$!; sleep 1.5; }
dienst_ende() { kill "$D" 2>/dev/null; wait "$D" 2>/dev/null; }
schema() { gsettings set org.gnome.desktop.interface color-scheme "$1"; sleep 1.5; }
# block_da DATEI FREMDKOPIE: Fremdes unverändert vorn, Block am Ende
block_da() { local n; n=$(wc -c <"$2"); cmp -s -n "$n" "$1" "$2" && grep -q "^$ANFANG" "$1" && tail -1 "$1" | grep -q '^# <<< querencia-sitzung$'; }
labwc_unberuehrt() { cmp -s "$TRC" "$QS_PROFILE/trc-fremd" && [ "$(labwc_stand)" = "$(cat "$QS_PROFILE/labwc-vorher")" ] && ! grep -qi 'SIGHUP\|themerc' "$DLOG"; }
foto_rahmen() { foot -o csd.preferred=server -W 60x6 -T "QD-088 Rahmen $1" sleep 30 >/dev/null 2>&1 & FT=$!; sleep 1.8; shot "fd-$1"; }
foto_gtk3() {
    python3 -I "$W/gtk3-fenster.py" "QD-103 $1" 3.5 >"$OUT/fd-$1.txt" 2>&1 & local p=$!
    sleep 2.5; shot "fd-$1"; wait $p
}
# soll BILD hell|dunkel TEXT PROGRAMM [BREITE HÖHE]
soll() {
    local m k; m=$($4 "$OUT/$1.png" ${5:-}); echo "INFO $1: $m" >>"$R"
    k=$(sed -n 's/.*Kontrast \([0-9.]*\):1.*/\1/p' <<<"$m")
    if [[ $m == *" $2, "* ]] && python3 -c "import sys; sys.exit(0 if ${k:-0} >= 4.5 else 1)"; then ok "$3: $2 ($m)"; else nok "$3: soll $2 ($m)"; fi
}

dienst_start
cmp -s "$INI" "$QS_PROFILE/ini-fremd" && grep -q 'Dunkelmodus (Start).*dunkel_aktiv false' "$DLOG" &&
    ok "F01 Start hell: settings.ini byte-gleich, Protokoll dunkel_aktiv false" || nok "F01 Start hell: $(tail -3 "$DLOG" | tr '\n' ';')"
foto_gtk3 gtk3-hell
soll fd-gtk3-hell hell "F02 GTK-3-Fenster hell (Bild fd-gtk3-hell)" "$GF" "400 120"

schema prefer-dark
block_da "$INI" "$QS_PROFILE/ini-fremd" && grep -q '^gtk-application-prefer-dark-theme=true$' "$INI" &&
    ok "F03 settings.ini: fremde Zeilen vorn byte-gleich, Block mit [Settings] prefer-dark am Ende" || nok "F03 settings.ini: $(tr '\n' '|' <"$INI")"
foto_gtk3 gtk3-dunkel
soll fd-gtk3-dunkel dunkel "F04 neu gestartetes GTK-3-Fenster dunkel (Bild fd-gtk3-dunkel)" "$GF" "400 120"
grep -q 'dunkel=1 grund=#3c3b37 schrift=#dfdbd2' "$OUT/fd-gtk3-dunkel.txt" && ok "F04b GTK 3 meldet prefer-dark=1, theme_bg #3c3b37, theme_fg #dfdbd2" || nok "F04b $(cat "$OUT/fd-gtk3-dunkel.txt")"
labwc_unberuehrt && ok "F05 labwc unberührt: themerc-override byte-gleich, keine neue Datei im Konfigordner, kein SIGHUP" || nok "F05 labwc berührt: $(ls "$XDG_CONFIG_HOME/labwc" | tr '\n' ' ') $(grep -i 'labwc\|themerc' "$DLOG" | tr '\n' ';')"
foto_rahmen rahmen-dunkel
soll fd-rahmen-dunkel hell "F05b labwc-Titelleiste bleibt bei dunkel hell (Bild fd-rahmen-dunkel)" "$RF"
kill $FT 2>/dev/null; wait $FT 2>/dev/null

schema default
cmp -s "$INI" "$QS_PROFILE/ini-fremd" && ok "F06 zurück auf hell: settings.ini wieder byte-gleich mit dem Fremdstand" || nok "F06 $(tr '\n' '|' <"$INI")"
foto_gtk3 gtk3-wieder-hell
soll fd-gtk3-wieder-hell hell "F07 GTK-3-Fenster danach wieder hell (Bild fd-gtk3-wieder-hell)" "$GF" "400 120"

gsettings set org.gnome.desktop.interface gtk-theme Ohne-Dunkel; schema prefer-dark
cmp -s "$INI" "$QS_PROFILE/ini-fremd" && grep -q 'Thema Ohne-Dunkel.*dunkel_aktiv false' "$DLOG" &&
    ok "F08 Thema ohne Dunkelfassung + prefer-dark: kein Block (dunkel_aktiv false)" || nok "F08 $(tail -2 "$DLOG" | tr '\n' ';')"
gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch; sleep 1.5
block_da "$INI" "$QS_PROFILE/ini-fremd" && ok "F08b Themenwechsel zurück auf Querencia-Klassisch bei prefer-dark: Block gesetzt" || nok "F08b"
dienst_ende

# F15: eigener Eintrag des Benutzers in [Settings] gilt, kein Block
printf '[Settings]\ngtk-application-prefer-dark-theme=false\n' >"$INI"; cp "$INI" "$QS_PROFILE/ini-eigen"
dienst_start
cmp -s "$INI" "$QS_PROFILE/ini-eigen" && grep -q 'setzt gtk-application-prefer-dark-theme selbst, der Eintrag des Benutzers gilt' "$DLOG" &&
    ok "F15 eigener Eintrag prefer-dark=false bei dunkel: kein Block, Datei byte-gleich, protokolliert" || nok "F15 $(tr '\n' '|' <"$INI") $(tail -2 "$DLOG" | tr '\n' ';')"
foto_gtk3 gtk3-eigen
soll fd-gtk3-eigen hell "F15b GTK-3-Fenster folgt dem Eintrag des Benutzers: hell (Bild fd-gtk3-eigen)" "$GF" "400 120"
schema default
cmp -s "$INI" "$QS_PROFILE/ini-eigen" && ok "F15c zurück auf hell: Datei mit eigenem Eintrag byte-gleich" || nok "F15c"
dienst_ende
schema prefer-dark

# F09: Start bei dunklem Schema, Datei nur mit unserem Block
rm -f "$INI"
dienst_start
grep -q "^$ANFANG" "$INI" && [ "$(head -1 "$INI")" = "$(grep "^$ANFANG" "$INI")" ] &&
    ok "F09 Start bei dunkel: settings.ini neu, nur mit dem Block" || nok "F09 $(tail -3 "$DLOG" | tr '\n' ';')"
[ "$(stat -c %a "$INI")" = 644 ] && ok "F09b settings.ini 0644" || nok "F09b Rechte $(stat -c %a "$INI")"
schema default
[ ! -e "$INI" ] && ok "F10 zurück auf hell: Datei, die nur den Block hatte, entfernt" || nok "F10 $(ls -l "$INI" 2>&1)"
ls "$(dirname "$INI")" | grep -q 'querencia-sitzung-neu' && nok "F10b Hilfsdatei liegen geblieben" || ok "F10b keine Hilfsdatei liegen geblieben"

# F11: settings.ini als Verweis, F12: beschädigter Block
printf '[Settings]\n' >"$QS_PROFILE/ziel.ini"; ln -s "$QS_PROFILE/ziel.ini" "$INI"
schema prefer-dark
[ -L "$INI" ] && [ "$(cat "$QS_PROFILE/ziel.ini")" = "[Settings]" ] && grep -q 'ist ein Verweis, bleibt unberührt' "$DLOG" &&
    ok "F11 settings.ini als Verweis: unberührt, protokolliert" || nok "F11 $(tail -3 "$DLOG" | tr '\n' ';')"
schema default
rm -f "$INI"
printf '[Settings]\ngtk-font-name=X 9\n%s (von der Sitzung verwaltet, nicht von Hand ändern)\ngtk-application-prefer-dark-theme=true\n' "$ANFANG" >"$INI"
cp "$INI" "$QS_PROFILE/ini-kaputt"
schema prefer-dark
cmp -s "$INI" "$QS_PROFILE/ini-kaputt" && grep -q 'Block ohne Ende, bleibt unberührt' "$DLOG" &&
    ok "F12 beschädigter Block: settings.ini unberührt, protokolliert" || nok "F12 $(tail -3 "$DLOG" | tr '\n' ';')"
r=$(busctl --user call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver GetActive 2>&1)
[ "$r" = "b false" ] && ok "F13 ScreenSaver-Dienst antwortet weiter (GetActive $r)" || nok "F13 GetActive: $r"
labwc_unberuehrt && ok "F14 am Ende: themerc-override und labwc-Konfigordner unverändert, kein SIGHUP im Protokoll" || nok "F14 labwc berührt"
schema default
dienst_ende
rm -f "$INI"
grep 'Dunkelmodus' "$DLOG" | sed 's/^/INFO Dienst: /' >>"$R"
echo "Ergebnis fremde Programme dunkel: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$QS_PROFILE/logs/FERTIG"
