#!/bin/bash
# Dunkelmodus für fremde GTK-3-Programme und die labwc-Rahmen (0.1.5,
# QD-103, QD-088 Punkt 6). Der Leerlaufdienst (`--leerlauf 600`) läuft im
# isolierten labwc; das Farbschema kommt wie live über gsettings und das
# Settings-Portal (portal-ersatz.py, gui-pruefung.sh setzt QS_PORTAL=1).
# Fremde Zeilen in settings.ini und themerc-override müssen byte-gleich
# bleiben. Fotos: foot-Fenster mit Serverdekoration (labwc-Titelleiste,
# rahmenfarbe.py) und werkzeug/gtk3-fenster.py (grundfarbe.py).
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
echo "INFO Thema $thema, labwc $(readlink /proc/$LABWC_PID/exe) -C $(tr '\0' ' ' </proc/$LABWC_PID/cmdline | sed 's/.* -C \([^ ]*\).*/\1/')" >>"$R"
# Thema ohne Dunkelfassung (F08): gtk.css ohne gtk-dark.css
for g in gtk-3.0 gtk-4.0; do mkdir -p "$XDG_DATA_HOME/themes/Ohne-Dunkel/$g"; : >"$XDG_DATA_HOME/themes/Ohne-Dunkel/$g/gtk.css"; done
python3 -I "$W/portal-ersatz.py" "$QS_PROFILE/logs/portal-bereit" >>"$OUT/fd-portal.log" 2>&1 &
for ((i = 0; i < 50; i++)); do [ -f "$QS_PROFILE/logs/portal-bereit" ] && break; sleep 0.1; done

# fremde Dateien mit eigenen Zeilen
printf '[Settings]\ngtk-font-name=DejaVu Sans 11\ngtk-application-prefer-dark-theme=false\n' >"$INI"
printf '# eigene Zeile des Benutzers\nwindow.button.spacing: 2\n' >"$TRC"
cp "$INI" "$QS_PROFILE/ini-fremd"; cp "$TRC" "$QS_PROFILE/trc-fremd"

dienst_start() { "$BIN" --leerlauf 600 >>"$DLOG" 2>&1 & D=$!; sleep 1.5; }
dienst_ende() { kill "$D" 2>/dev/null; wait "$D" 2>/dev/null; }
schema() { gsettings set org.gnome.desktop.interface color-scheme "$1"; sleep 1.5; }
# block_da DATEI FREMDKOPIE: Fremdes unverändert vorn, Block am Ende
block_da() { local n; n=$(wc -c <"$2"); cmp -s -n "$n" "$1" "$2" && grep -q "^$ANFANG" "$1" && tail -1 "$1" | grep -q '^# <<< querencia-sitzung$'; }
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
cmp -s "$INI" "$QS_PROFILE/ini-fremd" && cmp -s "$TRC" "$QS_PROFILE/trc-fremd" && grep -q 'Dunkelmodus (Start).*dunkel_aktiv false' "$DLOG" &&
    ok "F01 Start hell: settings.ini und themerc-override byte-gleich, Protokoll dunkel_aktiv false" || nok "F01 Start hell: $(tail -3 "$DLOG" | tr '\n' ';')"
# GTK-3-Fenster allein (mittig), danach das foot-Fenster, das über die
# Wechsel offen bleibt
foto_gtk3 gtk3-hell
soll fd-gtk3-hell hell "F03 GTK-3-Fenster hell (Bild fd-gtk3-hell)" "$GF" "400 120"
foto_rahmen rahmen-hell
soll fd-rahmen-hell hell "F02 labwc-Titelleiste hell (Bild fd-rahmen-hell)" "$RF"

schema prefer-dark
block_da "$INI" "$QS_PROFILE/ini-fremd" && grep -q '^gtk-application-prefer-dark-theme=true$' "$INI" &&
    ok "F04a settings.ini: fremde Zeilen vorn byte-gleich, Block mit [Settings] prefer-dark am Ende" || nok "F04a settings.ini: $(tr '\n' '|' <"$INI")"
block_da "$TRC" "$QS_PROFILE/trc-fremd" && grep -q '^window.active.title.bg.color: #3c3b37$' "$TRC" &&
    ok "F04b themerc-override: eigene Zeile vorn byte-gleich, Block am Ende" || nok "F04b themerc-override: $(tr '\n' '|' <"$TRC")"
grep -q 'lädt das Thema neu (SIGHUP)' "$DLOG" && ok "F04c labwc neu geladen (SIGHUP an LABWC_PID $LABWC_PID)" || nok "F04c kein SIGHUP: $(tail -3 "$DLOG" | tr '\n' ';')"
shot fd-rahmen-wechsel-dunkel
soll fd-rahmen-wechsel-dunkel dunkel "F05 offenes Fenster: labwc-Titelleiste nach dem Wechsel dunkel (Bild fd-rahmen-wechsel-dunkel)" "$RF"
kill $FT 2>/dev/null; wait $FT 2>/dev/null
foto_gtk3 gtk3-dunkel
soll fd-gtk3-dunkel dunkel "F06 neu gestartetes GTK-3-Fenster dunkel (Bild fd-gtk3-dunkel)" "$GF" "400 120"
grep -q 'dunkel=1 grund=#3c3b37 schrift=#dfdbd2' "$OUT/fd-gtk3-dunkel.txt" && ok "F06b GTK 3 meldet prefer-dark=1, theme_bg #3c3b37, theme_fg #dfdbd2" || nok "F06b $(cat "$OUT/fd-gtk3-dunkel.txt")"
foto_rahmen rahmen-dunkel
soll fd-rahmen-dunkel dunkel "F05b neu geöffnetes Fenster: labwc-Titelleiste dunkel (Bild fd-rahmen-dunkel)" "$RF"

schema default
cmp -s "$INI" "$QS_PROFILE/ini-fremd" && cmp -s "$TRC" "$QS_PROFILE/trc-fremd" &&
    ok "F07a zurück auf hell: beide Dateien wieder byte-gleich mit dem Fremdstand" || nok "F07a $(tr '\n' '|' <"$INI") / $(tr '\n' '|' <"$TRC")"
shot fd-rahmen-wechsel-hell
soll fd-rahmen-wechsel-hell hell "F07b offenes Fenster: labwc-Titelleiste wieder hell (Bild fd-rahmen-wechsel-hell)" "$RF"
kill $FT 2>/dev/null; wait $FT 2>/dev/null

gsettings set org.gnome.desktop.interface gtk-theme Ohne-Dunkel; schema prefer-dark
cmp -s "$INI" "$QS_PROFILE/ini-fremd" && cmp -s "$TRC" "$QS_PROFILE/trc-fremd" && grep -q 'Thema Ohne-Dunkel.*dunkel_aktiv false' "$DLOG" &&
    ok "F08 Thema ohne Dunkelfassung + prefer-dark: kein Block (dunkel_aktiv false)" || nok "F08 $(tail -2 "$DLOG" | tr '\n' ';')"
gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch; sleep 1.5
block_da "$INI" "$QS_PROFILE/ini-fremd" && block_da "$TRC" "$QS_PROFILE/trc-fremd" &&
    ok "F08b Themenwechsel zurück auf Querencia-Klassisch bei prefer-dark: Blöcke gesetzt" || nok "F08b"
dienst_ende

# F09: Start bei dunklem Schema, Dateien nur mit unserem Block
rm -f "$INI" "$TRC"
dienst_start
grep -q "^$ANFANG" "$INI" && grep -q "^$ANFANG" "$TRC" && [ "$(head -1 "$INI")" = "$(grep "^$ANFANG" "$INI")" ] &&
    ok "F09 Start bei dunkel: beide Dateien neu, nur mit dem Block" || nok "F09 $(tail -3 "$DLOG" | tr '\n' ';')"
[ "$(stat -c %a "$INI")" = 644 ] && ok "F09b settings.ini 0644" || nok "F09b Rechte $(stat -c %a "$INI")"
schema default
[ ! -e "$INI" ] && [ ! -e "$TRC" ] && ok "F10 zurück auf hell: Dateien, die nur den Block hatten, entfernt" || nok "F10 $(ls -l "$INI" "$TRC" 2>&1 | tr '\n' ';')"
ls "$(dirname "$INI")" "$(dirname "$TRC")" | grep -q 'querencia-sitzung-neu' && nok "F10b Hilfsdatei liegen geblieben" || ok "F10b keine Hilfsdatei liegen geblieben"

# F11: settings.ini als Verweis, F12: beschädigter Block
printf '[Settings]\n' >"$QS_PROFILE/ziel.ini"; ln -s "$QS_PROFILE/ziel.ini" "$INI"
printf 'a: 1\n%s (von Hand angefangen)\n' "$ANFANG" >"$TRC"
sed -i "s/ (von Hand angefangen)/ (von der Sitzung verwaltet, nicht von Hand ändern)/" "$TRC"
cp "$TRC" "$QS_PROFILE/trc-kaputt"
schema prefer-dark
[ -L "$INI" ] && [ "$(cat "$QS_PROFILE/ziel.ini")" = "[Settings]" ] && grep -q 'ist ein Verweis, bleibt unberührt' "$DLOG" &&
    ok "F11 settings.ini als Verweis: unberührt, protokolliert" || nok "F11 $(tail -3 "$DLOG" | tr '\n' ';')"
cmp -s "$TRC" "$QS_PROFILE/trc-kaputt" && grep -q 'Block ohne Ende, bleibt unberührt' "$DLOG" &&
    ok "F12 beschädigter Block: themerc-override unberührt, protokolliert" || nok "F12 $(tail -3 "$DLOG" | tr '\n' ';')"
r=$(busctl --user call org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver GetActive 2>&1)
[ "$r" = "b false" ] && ok "F13 ScreenSaver-Dienst antwortet weiter (GetActive $r)" || nok "F13 GetActive: $r"
schema default
dienst_ende
rm -f "$INI" "$TRC"
grep 'Dunkelmodus' "$DLOG" | sed 's/^/INFO Dienst: /' >>"$R"
echo "Ergebnis fremde Programme dunkel: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$QS_PROFILE/logs/FERTIG"
