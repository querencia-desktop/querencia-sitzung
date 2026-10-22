#!/bin/bash
# Dunkelmodus der Sitzungsdialoge (QD-109, 0.1.4): Dialog „Sitzung beenden“,
# Rückfragen „Bildschirm sperren“ und „Abmelden“, je gestartet bei hellem
# und bei dunklem Farbschema, dazu der Wechsel im laufenden Dialog. Das
# Farbschema kommt wie live über das Settings-Portal (portal-ersatz.py mit
# den gsettings des Wegwerfprofils; gui-pruefung.sh setzt QS_PORTAL=1).
# GTK-Thema: die Live-Ablage Querencia-Klassisch (nur gelesen, als Link).
# Grund- und Schriftfarbe misst grundfarbe.py im Foto.
. "$(dirname "$0")/gemeinsam.sh"
: >"$R"
W=$(dirname "$EINGABE")
GF="python3 -I $W/grundfarbe.py"
schrift=$(sed -n 's/^gtk-font-name=//p' "$XDG_CONFIG_HOME/gtk-4.0/settings.ini" 2>/dev/null)
[ -n "$schrift" ] && gsettings set org.gnome.desktop.interface font-name "$schrift"
gsettings set org.gnome.desktop.interface gtk-theme Querencia-Klassisch
thema=$(readlink -f "$XDG_DATA_HOME/themes/Querencia-Klassisch")
echo "INFO Thema $thema (gtk-dark.css: $([ -f "$thema/gtk-4.0/gtk-dark.css" ] && echo ja || echo nein)), Schrift ${schrift:-Vorgabe}" >>"$R"
python3 -I "$W/portal-ersatz.py" "$QS_PROFILE/logs/portal-bereit" >>"$OUT/dk-portal.log" 2>&1 &
for ((i = 0; i < 50; i++)); do [ -f "$QS_PROFILE/logs/portal-bereit" ] && break; sleep 0.1; done
[ -f "$QS_PROFILE/logs/portal-bereit" ] || { nok "Portal-Ersatz startet nicht"; touch "$QS_PROFILE/logs/FERTIG"; exit 1; }
schema() {
    case $1 in dunkel) gsettings set org.gnome.desktop.interface color-scheme prefer-dark ;;
    *) gsettings set org.gnome.desktop.interface color-scheme default ;; esac
    sleep 0.5
}
weg() { q "move 1270 710" "wait 200"; }
# prueft BILD SOLL(hell|dunkel) TEXT
pruefen() {
    local m; m=$($GF "$OUT/$1.png")
    echo "INFO $1: $m" >>"$R"
    local k; k=$(sed -n 's/.*Kontrast \([0-9.]*\):1.*/\1/p' <<<"$m")
    if [[ $m == *" $2, "* ]] && python3 -c "import sys; sys.exit(0 if $k >= 4.5 else 1)"; then
        ok "$3: $2 ($m)"
    else
        nok "$3: soll $2 ($m)"
    fi
}

for s in hell dunkel; do
    schema $s
    start "dk-$s-dialog" --dialog; weg; shot "dk-$s-dialog"
    q "key Escape" "wait 800"; ende
    pruefen "dk-$s-dialog" $s "Dialog „Sitzung beenden“, gestartet $s (Bild dk-$s-dialog)"
    for a in sperren abmelden; do
        START_WARTEN=1.5 start "dk-$s-$a" "--$a"; weg; shot "dk-$s-$a"
        q "key Escape" "wait 800"; ende
        [ $RC = 0 ] && grep -q abgebrochen "$OUT/dk-$s-$a.log" || nok "Rückfrage --$a $s: Exit $RC"
        pruefen "dk-$s-$a" $s "Rückfrage --$a, gestartet $s (Bild dk-$s-$a)"
    done
done

# Wechsel im laufenden Dialog: dunkel → hell → dunkel
schema dunkel
start dk-wechsel --dialog; weg
schema hell; sleep 0.5; shot dk-wechsel-hell
schema dunkel; sleep 0.5; shot dk-wechsel-dunkel
q "key Escape" "wait 800"; ende
pruefen dk-wechsel-hell hell "Wechsel im laufenden Dialog nach hell (Bild dk-wechsel-hell)"
pruefen dk-wechsel-dunkel dunkel "Wechsel im laufenden Dialog zurück nach dunkel (Bild dk-wechsel-dunkel)"
grep -h 'Farbschema' "$OUT"/dk-*.log >>"$R" 2>/dev/null
schema hell
echo "Ergebnis Dunkelmodus: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$QS_PROFILE/logs/FERTIG"
