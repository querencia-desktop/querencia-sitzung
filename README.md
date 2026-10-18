# Querencia Sitzung

Sitzungsfunktionen eines klassischen Desktops für Querencia Desktop
(labwc 0.9.8, GTK 4, ohne libadwaita): Abmelden, Bildschirm sperren,
Neu starten, Herunterfahren und Leerlaufsperre. Auftrag G2-K.

```text
querencia-sitzung --dialog [--countdown SEK | --kein-countdown]
querencia-sitzung --abmelden | --neustart | --herunterfahren [--countdown SEK | --kein-countdown]
querencia-sitzung --sperren [--sofort]
querencia-sitzung --leerlauf MINUTEN | --leerlauf-sekunden SEK
querencia-sitzung --version | --hilfe
```

| Teil | Technik | Datei |
|---|---|---|
| Rückfragen, Gesamtdialog | GTK 4, GNOME-2-Aufbau, labwc-Rahmen; Esc, Pfeile, Enter, Mnemoniken; Countdown 60 s bei Einzelrückfragen | `src/dialog.rs` |
| Abmelden | `<labwc> --exit` mit dem labwc aus `/proc/$LABWC_PID/exe` (wie das Panel) | `src/aktion.rs` |
| Neu starten, Herunterfahren | systemd-logind D-Bus `CanReboot`/`CanPowerOff`, dann `Reboot`/`PowerOff` (interactive=true, polkit darf fragen) | `src/aktion.rs` |
| Sperrbildschirm | ext-session-lock-v1 direkt über wayland-client (kein GTK), Cairo/Pango, eine Fläche je Ausgang, Uhr, Passwortfeld | `src/sperre.rs` |
| Passwort | PAM über `dlopen("libpam.so.0")` (keine PAM-Entwicklungsdateien auf der VM), Dienst `querencia-sitzung` falls in `/etc/pam.d`, sonst `login` | `src/pam.rs` |
| Leerlauf | ext-idle-notify-v1, startet `--sperren --sofort` | `src/leerlauf.rs` |

`QS_TROCKENLAUF=1`: Abmelden, Neu starten und Herunterfahren werden nur
protokolliert. Zusammen mit `QS_TESTPASSWORT` und einem `XDG_RUNTIME_DIR`
außerhalb `/run/user/` entsperrt der Sperrbildschirm ohne PAM (nur in der
isolierten Prüfung möglich).

## Bauen und prüfen

```bash
cargo build --release          # target/release/querencia-sitzung
cargo test                     # Unit-Tests (ohne Wayland)
bash werkzeug/gui-pruefung.sh  # isolierte GUI-Prüfung (eigener headless labwc)
```

Die GUI-Prüfung berührt die laufende Sitzung nicht (eigenes
`XDG_RUNTIME_DIR`, eigener D-Bus, eigenes Profil unter `gui-test/`);
Ergebnisse und Bilder liegen unter `nachweis/gui/`. `QS_BIN=…` wählt das
geprüfte Programm (Standard `target/debug`).

## Integration

Vorbereitet, nicht angewendet, unter `integration/`:
`sitzung-integrieren.sh pruefen|aktivieren|zurueck` (Programm-Link,
Menüeintrag, Symbole), Patches für `rc.xml` (Strg+Alt+L, Strg+Alt+Entf,
Fensterregel „mittig“), für `querencia-session`/`autostart`
(Leerlaufsperre) und für das Panel-Systemmenü sowie ein PAM-Dienstvorschlag.

Symbolregel: kein `hicolor/48x48` neben Scalable-Symbolen (GTK-4.22-Absturz).
