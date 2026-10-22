#!/bin/bash
# Messung QD-103 (0.1.5): Woher nimmt ein fremdes GTK-3-Programm (GTK 3.24,
# Wayland) Thema und Dunkelfassung? Nur gemessen, nichts von der Sitzung
# beteiligt. Das Programm ist werkzeug/gtk3-fenster.py; Quellen:
#   - Portal-Ersatz (org.freedesktop.portal.Settings) mit den gsettings des
#     Wegwerfprofils: color-scheme, gtk-theme
#   - $XDG_CONFIG_HOME/gtk-3.0/settings.ini
# Jeder Fall: Programmausgabe (Thema, prefer-dark, theme_bg_color) und Foto.
. "$(dirname "$0")/gemeinsam.sh"
: >"$R"
W=$(dirname "$EINGABE")
F3="python3 -I $W/gtk3-fenster.py"
GF="python3 -I $W/grundfarbe.py"
INI=$XDG_CONFIG_HOME/gtk-3.0/settings.ini
mkdir -p "$(dirname "$INI")"
gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch
gsettings set org.gnome.desktop.interface color-scheme default
thema=$(readlink -f "$XDG_DATA_HOME/themes/Querencia-Klassisch")
echo "INFO Thema $thema (gtk-3.0/gtk-dark.css: $([ -f "$thema/gtk-3.0/gtk-dark.css" ] && echo ja || echo nein))" >>"$R"
python3 -I "$W/portal-ersatz.py" "$QS_PROFILE/logs/portal-bereit" >>"$OUT/g3m-portal.log" 2>&1 &
for ((i = 0; i < 50; i++)); do [ -f "$QS_PROFILE/logs/portal-bereit" ] && break; sleep 0.1; done
echo "INFO Portal-Ersatz: $(cat "$QS_PROFILE/logs/portal-bereit" 2>/dev/null), GTK_USE_PORTAL=${GTK_USE_PORTAL:-ungesetzt}, GDK_DEBUG=${GDK_DEBUG:-ungesetzt}" >>"$R"

# fall NAME BESCHREIBUNG [Befehle während des Laufs (nach 1,5 s)]
fall() {
    local n=$1 text=$2 mitte=${3:-}
    $F3 "QD-103 $n" 5 >"$OUT/g3m-$n.txt" 2>&1 & local p=$!
    sleep 1.5
    [ -n "$mitte" ] && eval "$mitte"
    sleep 2
    grim "$OUT/g3m-$n.png"
    wait $p
    echo "MESSUNG $n $text" >>"$R"
    sed 's/^/    /' "$OUT/g3m-$n.txt" >>"$R"
    echo "    Foto: $($GF "$OUT/g3m-$n.png" 400 120)" >>"$R"
}

rm -f "$INI"
fall m1 "keine settings.ini, color-scheme default"
gsettings set org.gnome.desktop.interface color-scheme prefer-dark; sleep 0.3
fall m2 "keine settings.ini, color-scheme prefer-dark (Portal meldet 1)"
gsettings set org.gnome.desktop.interface color-scheme default; sleep 0.3
printf '[Settings]\ngtk-application-prefer-dark-theme=true\n' >"$INI"
fall m3 "settings.ini prefer-dark=true beim Start"
rm -f "$INI"
fall m4 "settings.ini erst im Lauf geschrieben" "printf '[Settings]\ngtk-application-prefer-dark-theme=true\n' >'$INI'"
fall m5 "settings.ini prefer-dark im Lauf, danach gtk-theme Adwaita und zurück" \
    "gsettings set org.gnome.desktop.interface gtk-theme Adwaita; sleep 0.7; gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch"
printf '[Settings]\ngtk-application-prefer-dark-theme=false\ngtk-font-name=DejaVu Sans 11\n\n[Andere]\nx=1\n# >>> querencia-sitzung\n[Settings]\ngtk-application-prefer-dark-theme=true\n# <<< querencia-sitzung\n' >"$INI"
fall m6 "fremder Abschnitt prefer-dark=false, dazwischen [Andere], am Ende Block mit erneutem [Settings] prefer-dark=true"
cp "$INI" "$OUT/g3m-m6-settings.ini"
printf '[Settings]\ngtk-application-prefer-dark-theme=true\n' >"$INI"
fall m7 "settings.ini prefer-dark, Wechsel gtk-theme im Lauf auf Querencia-Klassisch:dark" \
    "gsettings set org.gnome.desktop.interface gtk-theme 'Querencia-Klassisch:dark'; sleep 0.7; gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch"
rm -f "$INI"
GTK_USE_PORTAL=1 fall m8 "keine settings.ini, GTK_USE_PORTAL=1, Wechsel gtk-theme im Lauf Adwaita" \
    "gsettings set org.gnome.desktop.interface gtk-theme Adwaita"
gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch
GDK_DEBUG=no-portals fall m9 "keine settings.ini, GDK_DEBUG=no-portals, Wechsel gtk-theme im Lauf Adwaita" \
    "gsettings set org.gnome.desktop.interface gtk-theme Adwaita"
gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch
rm -f "$INI"
fall m10 "ohne settings.ini gestartet, im Lauf settings.ini prefer-dark, danach gtk-theme Adwaita und zurück" \
    "printf '[Settings]\\ngtk-application-prefer-dark-theme=true\\n' >'$INI'; sleep 0.3; gsettings set org.gnome.desktop.interface gtk-theme Adwaita; sleep 0.7; gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch"
rm -f "$INI"
echo "Messung GTK 3 fertig" >>"$R"
touch "$QS_PROFILE/logs/FERTIG"
