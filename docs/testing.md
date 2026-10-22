# Testen

```bash
cargo fmt --all --check
env -u WAYLAND_DISPLAY -u DISPLAY cargo clippy --all-targets --locked -- -D warnings
env -u WAYLAND_DISPLAY -u DISPLAY cargo test --locked
python3 werkzeug/i18n.py pruefen          # .pot aktuell, de.po vollständig
```

`falsches_passwort_wird_abgelehnt` ruft echtes PAM (Dienst `login`) mit dem
eigenen Benutzer und einem falschen Passwort auf; das zählt als Fehlversuch
des echten Kontos. Der Test ist deshalb mit `#[ignore]` markiert und läuft
nur bewusst, in einer Wegwerf-Umgebung (Test-VM, Container mit eigenem
Konto):

```bash
env -u WAYLAND_DISPLAY -u DISPLAY cargo test --locked -- --ignored falsches_passwort_wird_abgelehnt
```

`logind_can_reboot_antwortet` fragt `CanReboot` am Systembus (nur lesend).

GUI nur **isoliert**: eigener headless labwc 0.9.8, eigener D-Bus, eigenes
XDG-Profil, `XDG_RUNTIME_DIR` unter `laufzeit/qed-runtime-*`, immer
`QS_TROCKENLAUF=1` (Abmelden, Neu starten, Herunterfahren werden nur
protokolliert). Die Szenarien prüfen selbst, dass `LABWC_PID` der isolierte
labwc ist.

`qeingabe`/`qausgabe` bauen (Quellen und erzeugte Protokolldateien liegen im
Repo):

```bash
cd werkzeug && mkdir -p bin && \
gcc -O2 -o bin/qeingabe qeingabe.c protokolle/virtual-keyboard-unstable-v1-protocol.c \
    protokolle/wlr-virtual-pointer-unstable-v1-protocol.c -Iprotokolle \
    $(pkg-config --cflags --libs wayland-client xkbcommon) && \
gcc -O2 -o bin/qausgabe qausgabe.c protokolle/wlr-output-management-unstable-v1-protocol.c \
    -Iprotokolle $(pkg-config --cflags --libs wayland-client) && cd ..
```

**Regression** (alle Szenarien, ca. 10 min):

```bash
QS_BIN=<ordner>/querencia-sitzung bash werkzeug/gui-pruefung.sh
```

`QS_OHNE_SYSTEM=1` ist die Vorgabe: Fälle, die das echte System fragen,
werden als BLOCKED ausgelassen – D05–D07 und D12 (logind auf dem
Systembus), S08 (echtes PAM) und das ganze polkit-Szenario (registriert den
Agenten bei der echten logind-Sitzung). `QS_OHNE_SYSTEM=0` nur mit
ausdrücklicher Freigabe.

**Sprache** (`szenario-sprache.sh`, ca. 1 min): Katalog eingeblendet
(`/proc/<pid>/maps`), Dialog „Sitzung beenden“ und die vier Rückfragen nur
öffnen und mit Esc abbrechen, Zugriffstaste „Abmelden“ der Sprache im
Trockenlauf:

```bash
QS_BIN=<ordner>/querencia-sitzung QS_SPRACHE=de QS_GROESSE=1024x768 \
    bash werkzeug/gui-pruefung.sh szenario-sprache.sh 1 120
QS_BIN=<ordner>/querencia-sitzung QS_SPRACHE=en QS_GROESSE=1920x1080 \
    bash werkzeug/gui-pruefung.sh szenario-sprache.sh 1 120
```

Für `LANGUAGE=de` braucht die Binary den Katalog daneben
(`<ordner>/share/locale/de/LC_MESSAGES/querencia-sitzung.mo`). Ergebnis in
`nachweis/gui/ergebnis.txt`, Bilder und Programmprotokolle daneben; die Läufe
überschreiben die eingecheckten Belege, danach
`git checkout -- nachweis/ && git clean -fq nachweis/gui/`.

## Rückfragen ohne Countdown, Abmelden (0.1.3, G2-AI, QD-062)

```bash
QS_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-g2ai.sh 1 220           # C01–C08, Trockenlauf
QS_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-g2ai-abmelden.sh 1 60   # A02a–e, echtes Abmelden im isolierten labwc
```

- **C04/C05 (Vorgabe):** kein Countdown. Die Rückfrage wartet; Enter auf dem
  Standardknopf „Abbrechen“ löst nichts aus.
- **C01–C03, C06, C07 (nur auf Wunsch):**
  - Der Countdown gilt mit `--countdown` oder `sitzung.ini` `Countdown=an`.
  - Er zählt nur mit Fokus. Ein kleines `foot`-Fenster („fokusdieb“) nimmt
    den Fokus.
  - Nach 60 s ohne Fokus bricht der Dialog ab.
- **C08:** `--kein-countdown` endet mit Exit 2, die Option gibt es nicht mehr.
- **A02:**
  - Der Dialog beendet fünf Sitzungsprogramme, darunter Platzhalter aus
    `werkzeug/platzhalter.c`; einer ignoriert SIGTERM.
  - Danach stoppt er die Portale. Im isolierten Lauf erreicht
    `systemctl --user` keinen User-Manager; das wird nur protokolliert.
  - Erst dann folgt `labwc --exit`.
  - Die Köder bleiben: dasselbe Programm mit anderer `LABWC_PID` und ein
    Benutzerprogramm.
- Ohne Eingabegerät hat der headless labwc keine Tastatur am Sitz, dann wird
  kein Fenster aktiv. `eingabe.sh` steckt eine virtuelle Tastatur an.

## ScreenSaver-Dienst, Dunkelmodus, Emoji im Titel (0.1.4)

Liegt das Repo so tief, dass der Pfad des Wayland-Sockets länger als 108
Zeichen wird (labwc: „File name too long“), setzt `QS_LAUFZEIT` einen
kürzeren Ordner für die Laufzeitordner, z. B. `QS_LAUFZEIT=~/.cache/qs`.

```bash
QS_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-schoner.sh 1 330   # S01–S15, ca. 2 min
QS_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-dunkel.sh 1 200    # 8 Fälle, ca. 30 s
QS_LANG=C.UTF-8 QS_RC_XML=<querencia-runtime>/session/querencia/labwc/rc.xml \
    QS_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-emoji.sh 1 150 # 11 Titel, Fotos
```

- **ScreenSaver (S01–S15):**
  - Alles läuft am Wegwerf-Bus von `dbus-run-session`. Das Szenario bricht
    ab, wenn `DBUS_SESSION_BUS_ADDRESS` auf `/run/user/` zeigt.
  - Der Leerlauf beträgt 8 s.
  - `werkzeug/schoner-client.py` hält Hemmungen über seine eigene
    Busverbindung. `busctl --user` ruft die übrigen Methoden.
    `gdbus monitor` zeichnet `ActiveChanged` auf.
  - S14 startet `xdg-desktop-portal-gtk` im Wegwerf-Bus. Es hemmt über
    `org.freedesktop.impl.portal.Inhibit` (Flag 8, Leerlauf) und hebt mit
    `Request.Close` wieder auf.
  - Belege: `schoner-busctl.txt`, `schoner-monitor.txt`,
    `schoner-dienst.log`, `schoner-portal.log`.
- **Dunkelmodus:** `gui-pruefung.sh` setzt für dieses Szenario `QS_PORTAL=1`.
  - GTK fragt dann das Settings-Portal. Den Namen belegt
    `werkzeug/portal-ersatz.py` mit den gsettings des Wegwerfprofils.
  - Das Thema ist die Live-Ablage Querencia-Klassisch, nur gelesen und als
    Link eingebunden.
  - `werkzeug/grundfarbe.py` misst im Foto Grund- und Schriftfarbe um die
    Bildmitte und den Kontrast nach WCAG 2 (Soll ≥ 4,5:1).
- **Emoji im Titel:** Für jeden Titel aus `werkzeug/emoji-titel.py` öffnet
  das Szenario ein `foot`-Fenster mit Serverdekoration.
  `werkzeug/titelleisten.py` stapelt die Titelleisten zu einem Bild.
  - Grenze: labwc zeichnet Titel mit Pango und dem Fallback von
    fontconfig. Kästchen mit Hex-Ziffern erscheinen für Zeichen, die keine
    installierte Schrift hat. Auf der Entwicklungs-VM (nur DejaVu, FreeFont,
    Noto Color Emoji) sind das ganze Schriften: CJK (Chinesisch, Japanisch,
    Koreanisch), Vollbreite-Formen (U+FF00–U+FFEF), dazu Teile von Arabisch
    (Darstellungsformen-A, z. B. U+FDFD, Extended-B), Symbols for Legacy
    Computing und Private Use. Der Befund vom 22.10. („4K #“ und rund 20
    Kästchen, Fall t12/t13) ist so nachgestellt. Abhilfe sind weitere
    Schriften im System (etwa Noto CJK), nicht die `rc.xml`.

## Dunkelmodus für fremde GTK-3-Programme (0.1.5)

```bash
QS_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-gtk3-messung.sh 1 150 # Messung m1–m10
QS_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-fremd-dunkel.sh 1 240  # F01–F14
```

- Beide Szenarien laufen mit dem Portal-Ersatz (`QS_PORTAL=1`); das
  Farbschema schalten sie über die gsettings des Wegwerfprofils.
- **Messung GTK 3:** `werkzeug/gtk3-fenster.py` öffnet ein GTK-3-Fenster
  über ctypes (`libgtk-3.so.0`; GTK-3-Introspection und `gtk3-demo` fehlen
  auf der VM) und meldet `gtk-theme-name`,
  `gtk-application-prefer-dark-theme` und die Themenfarben. Befund:
  - Portal-Farbschema allein: hell (m2).
  - `settings.ini` mit `prefer-dark` beim Start: dunkel, Kontrast 8,1:1
    (m3).
  - `settings.ini` erst im Lauf geschrieben: hell, auch nach einem
    Themenwechsel (m4, m10).
  - Ein zweites `[Settings]` am Dateiende geht früheren Einträgen vor (m6).
  - `gtk-theme` folgt live, mit und ohne Portal (m8, m9); `Thema:dark`
    kennt GTK 3 nicht (m7).
- **Fremde Programme dunkel:**
  - Der Leerlaufdienst (`--leerlauf 600`) läuft im isolierten labwc.
  - `settings.ini` beginnt mit fremden Zeilen, die byte-gleich bleiben
    müssen.
  - Gegenprobe labwc: Ein fremdes `themerc-override` im Konfigordner des
    isolierten labwc bleibt byte-gleich, es entsteht keine neue Datei, das
    Protokoll nennt kein SIGHUP, die Titelleiste bleibt bei dunkel hell
    (`rahmenfarbe.py`).
  - Weitere Fälle: Thema ohne Dunkelfassung, Start bei dunkel, Datei nur
    mit Block, Verweis, beschädigter Block, ScreenSaver antwortet weiter,
    eigener Eintrag des Benutzers gilt (F15).
- **Integration:** `probe-015.sh` prüft zusätzlich (7a–7g), dass `zurueck`
  genau den verwalteten Block aus `settings.ini` nimmt: Fehlte die Datei,
  ist sie danach weg; fremder Inhalt ist byte-gleich zur Sicherung von
  `aktivieren`; Fehler rollen auch die Datei zurück; Verweis und
  beschädigter Block bleiben unberührt.

## Bildschirm aus nach der Sperre (0.1.6)

```bash
QS_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-bildschirm.sh 1 300   # B01–B15, ca. 2 min
```

- Wegwerf-Bus wie beim ScreenSaver-Szenario; das Szenario bricht ab, wenn
  der Bus unter `/run/user/` liegt.
- `sitzung.ini` des Profils mit `[Sperre]` `BildschirmAus=4`; der
  Leerlaufdienst sperrt selbst erst nach 300 s, gesperrt wird über
  `org.freedesktop.ScreenSaver.Lock` bzw. wie vom Tastenkürzel.
- Ob der Ausgang an ist, misst `grim -o HEADLESS-1`: Ein abgeschalteter
  Ausgang lässt sich nicht fotografieren. labwc bestätigt den Wechsel an
  die Steuerung des Dienstes („Ausgang meldet aus“). wlr-output-management
  meldet ihn nicht: labwc schickt dabei kein neues `enabled` (gemessen).
- Fälle: aus nach 4 s, Eingabe → an (Sperrbildschirm sichtbar), wieder
  aus, Entsperren, entsperrt nie aus, Hemmung hält an, UnInhibit zählt von
  vorn, neue Hemmung → an, SimulateUserActivity, Sperre vom Tastenkürzel,
  `BildschirmAus=0`, Vorgabe 120 s ohne Datei.
- NOT TESTED: Wayland-Hemmung (idle-inhibit) eines Videoprogramms, echte
  Monitore (DPMS) statt headless.
