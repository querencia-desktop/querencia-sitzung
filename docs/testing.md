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
