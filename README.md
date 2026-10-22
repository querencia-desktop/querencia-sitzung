# Querencia Sitzung

Sitzungsfunktionen eines klassischen Desktops für Querencia Desktop
(labwc 0.9.8, GTK 4, ohne libadwaita): Abmelden, Bildschirm sperren,
Neu starten, Herunterfahren und Leerlaufsperre, dazu der polkit-Agent
`querencia-polkit-agent`.

> **Alpha.** Sperrbildschirm und polkit-Agent sind nicht unabhängig
> sicherheitsgeprüft. Keine Sicherheits- oder Datenverlustgarantie.

## Stand

| | |
|---|---|
| Version | 0.1.6 (Tags `sitzung/0.1.6`, `sitzung/0.1.6-int`): Bildschirm aus nach der Sperre; davor 0.1.5: fremde GTK-3-Programme folgen dem Dunkelmodus (QD-103) |
| Auf der Entwicklungs-VM | 0.1.5 aktiviert am 22.10.2026 21:10 (Dienst ab der nächsten Anmeldung); 0.1.6 wartet auf Freigabe |
| Letzte Prüfung | 22.10.2026 (0.1.6): fmt, Clippy, 67 + 10 Unit-Tests; isolierte Regression 33 PASS / 0 FAIL / 4 BLOCKED (echte logind/PAM/polkit bewusst ausgelassen), Countdown 9/9, Abmelden 10/10, Sprache Deutsch und Englisch je 7/7, ScreenSaver-Dienst 15/15, Dunkelmodus 8/8, fremde GTK-3-Programme 22/22, Bildschirm aus 18/18 |
| Getestet auf | Ubuntu 26.04, labwc 0.9.8, GTK 4.22 |

Änderungen je Version: [CHANGELOG.md](CHANGELOG.md).

## Befehle und Aufbau

```text
querencia-sitzung --dialog [--countdown SEK]
querencia-sitzung --abmelden | --neustart | --herunterfahren [--countdown SEK]
querencia-sitzung --sperren [--sofort]
querencia-sitzung --leerlauf MINUTEN | --leerlauf-sekunden SEK
querencia-sitzung --version | --hilfe
```

| Teil | Technik | Datei |
|---|---|---|
| Rückfragen, Gesamtdialog | GTK 4, GNOME-2-Aufbau, labwc-Rahmen; Esc, Pfeile, Enter, Mnemoniken; seit 0.1.3 ohne Countdown (nur auf Wunsch: `--countdown SEK` oder `sitzung.ini` `[Dialog]` `Countdown=an`) | `src/dialog.rs` |
| Abmelden | `<labwc> --exit` mit dem labwc aus `/proc/$LABWC_PID/exe` (wie das Panel) | `src/aktion.rs` |
| Neu starten, Herunterfahren | systemd-logind D-Bus `CanReboot`/`CanPowerOff`, dann `Reboot`/`PowerOff` (interactive=true, polkit darf fragen) | `src/aktion.rs` |
| Sperrbildschirm | ext-session-lock-v1 direkt über wayland-client (kein GTK), Cairo/Pango, eine Fläche je Ausgang, Uhr, Passwortfeld | `src/sperre.rs` |
| Passwort | PAM über `dlopen("libpam.so.0")` (keine PAM-Entwicklungsdateien auf der VM), Dienst `querencia-sitzung` falls in `/etc/pam.d`, sonst `login` | `src/pam.rs` |
| Leerlauf | ext-idle-notify-v1, startet `--sperren --sofort`; bietet `org.freedesktop.ScreenSaver` an (Inhibit hält die Sperre an, Lock, GetActive, ActiveChanged), GLib-Hauptschleife | `src/leerlauf.rs`, `src/schoner.rs` |
| Bildschirm aus | gesperrt und `BildschirmAus` (Vorgabe 120 s, `sitzung.ini` `[Sperre]`, 0 = nie) ohne Eingabe: Ausgänge aus über wlr-output-power-management, Eingabe oder Entsperren schaltet an; ScreenSaver-Hemmung hält an | `src/bildschirm.rs`, `src/leerlauf.rs` |
| Dunkelmodus | Farben nur aus dem GTK-Thema; Einstellung `gtk-interface-color-scheme` nach dem Start neu melden (GTK-4.22-Startfehler) | `src/farbschema.rs` |
| Dunkelmodus für GTK 3 | Leerlaufdienst schaltet bei `dunkel_aktiv` fremde GTK-3-Programme mit (verwalteter Block in `~/.config/gtk-3.0/settings.ini`, wirkt beim nächsten Programmstart); fremde Zeilen bleiben; labwc-Rahmen nicht (Sache von Thema und Einstellungen) | `src/mitschalten.rs` |

`QS_TROCKENLAUF=1`: Abmelden, Neu starten und Herunterfahren werden nur
protokolliert. Zusammen mit `QS_TESTPASSWORT` und einem `XDG_RUNTIME_DIR`
außerhalb `/run/user/` entsperrt der Sperrbildschirm ohne PAM (nur in der
isolierten Prüfung möglich).

## Bauen und Prüfen

`cargo build --release --locked`, danach `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` und `python3 werkzeug/i18n.py pruefen`.
Katalog, PAM-Dienst, Integration und die isolierte GUI-Prüfung (eigener labwc, eigener D-Bus, eigenes Profil): [docs/building.md](docs/building.md), [docs/testing.md](docs/testing.md).

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

GNU GPL, Version 3 oder neuer, siehe [LICENSE](LICENSE) (reiner Lizenztext).
Copyright 2026 Querencia Desktop contributors; Urheberhinweis in
[COPYRIGHT](COPYRIGHT). Eigener Code; aus gnome-session,
gnome-screensaver und polkit-gnome wurde kein Code übernommen. Änderungen:
[CHANGELOG.md](CHANGELOG.md).

Eigene Vermerke: die mit `wayland-scanner` erzeugten Protokolldateien unter
`werkzeug/protokolle/` (virtual-keyboard: Copyright Kristian Høgsberg u. a.;
wlr-Protokolle: jeweilige Autoren; MIT-artige Lizenzen im Dateikopf); nur
für das Prüfwerkzeug.

