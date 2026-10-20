# Querencia Sitzung

Sitzungsfunktionen eines klassischen Desktops für Querencia Desktop
(labwc 0.9.8, GTK 4, ohne libadwaita): Abmelden, Bildschirm sperren,
Neu starten, Herunterfahren und Leerlaufsperre, dazu der polkit-Agent
`querencia-polkit-agent`. Version 0.1.2.

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

Bauen: [docs/building.md](docs/building.md). Prüfen (Unit-Tests, isolierte
GUI mit eigenem labwc, eigenem D-Bus und eigenem Profil):
[docs/testing.md](docs/testing.md).

Gestartet werden der Leerlaufdienst (`--leerlauf`) und der polkit-Agent vom
Autostart der Querencia-Sitzung (`querencia-runtime`), die Dialoge über
labwc-Tastenkürzel (Strg+Alt+L, Strg+Alt+Entf) und das Systemmenü des Panels.

Symbolregel: kein `hicolor/48x48` neben Scalable-Symbolen (GTK-4.22-Absturz).

## Sprachen

Seit 0.1.2 sind die Oberflächentexte englisch im Code (msgid) und deutsch in
`po/de.po` (GNU gettext, Muster Querencia Editor 0.2.0); der deutsche
Wortlaut ist der bisherige. Englischer Wortschatz nach gnome-session 2.30
(„Log Out“, „Lock Screen“, „Restart“, „Shut Down“, Countdown „You will be
automatically logged out in 60 seconds.“) und polkit-gnome („Authenticate“,
„Password for …:“). Übersetzt sind Dialoge, Rückfragen, Countdown,
Fehlermeldungen der Aktionen und der polkit-Agent. **Noch deutsch** sind der
Sperrbildschirm (`sperre.rs`, Datum aus `zeit.rs`) und die PAM-Meldungen
(`pam.rs`): Änderungen dort nur nach Rückfrage. Beide Programme teilen den
Katalog `querencia-sitzung.mo`. Die Sprache folgt `LANGUAGE`/`LANG`; ohne
deutschen Katalog erscheint Englisch. Befehlszeile, `--hilfe`, Protokoll und
die D-Bus-Fehlerantworten des Agenten an polkitd bleiben deutsch.

## Lizenz

GNU GPL, Version 3 oder neuer, siehe [LICENSE](LICENSE). Copyright 2026
Querencia Desktop contributors. Eigener Code; aus gnome-session,
gnome-screensaver und polkit-gnome wurde kein Code übernommen. Änderungen:
[CHANGELOG.md](CHANGELOG.md).

Eigene Vermerke: die mit `wayland-scanner` erzeugten Protokolldateien unter
`werkzeug/protokolle/` (virtual-keyboard: Copyright Kristian Høgsberg u. a.;
wlr-Protokolle: jeweilige Autoren; MIT-artige Lizenzen im Dateikopf); nur
für das Prüfwerkzeug.

