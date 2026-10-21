# Querencia Sitzung

Sitzungsfunktionen eines klassischen Desktops für alle, die Querencia Desktop
benutzen: Abmelden, Bildschirm sperren, Neu starten, Herunterfahren,
Leerlaufsperre und ein polkit-Legitimierungsagent. Teil von
[Querencia Desktop](https://github.com/querencia-desktop/querencia) – einer
klassischen Desktopumgebung im GNOME-2-Stil für Wayland (labwc, Rust, GTK 4).

*English summary:* Log out, lock screen, restart, shut down, idle lock and a
polkit authentication agent for labwc, modelled on the GNOME 2 session
dialogs. Part of Querencia Desktop, a classic GNOME-2-style desktop for
Wayland. Alpha software; documentation is in German.

> **Alpha.** Entsperren mit dem echten Passwort und eine erfolgreiche
> polkit-Legitimierung sind nur von Hand geprüft, nicht automatisch; es gibt
> noch keinen eigenen PAM-Dienst. Keine Sicherheits- oder
> Datenverlustgarantie – der Sperrbildschirm ist keine geprüfte
> Sicherheitskomponente.

## Zweck

```text
querencia-sitzung --dialog [--countdown SEK]
querencia-sitzung --abmelden | --neustart | --herunterfahren [--countdown SEK]
querencia-sitzung --sperren [--sofort]
querencia-sitzung --leerlauf MINUTEN | --leerlauf-sekunden SEK
querencia-sitzung --version | --hilfe
querencia-polkit-agent
```

| Teil | Technik | Datei |
|---|---|---|
| Rückfragen, Gesamtdialog | GTK 4, GNOME-2-Aufbau, labwc-Rahmen; Esc, Pfeile, Enter, Mnemoniken; seit 0.1.3 ohne Countdown (nur auf Wunsch: `--countdown SEK` oder `sitzung.ini` `[Dialog]` `Countdown=an`) | `src/dialog.rs` |
| Abmelden | `<labwc> --exit` mit dem labwc aus `/proc/$LABWC_PID/exe` (wie das Panel) | `src/aktion.rs` |
| Neu starten, Herunterfahren | systemd-logind D-Bus `CanReboot`/`CanPowerOff`, dann `Reboot`/`PowerOff` (interactive=true, polkit darf fragen) | `src/aktion.rs` |
| Sperrbildschirm | ext-session-lock-v1 direkt über wayland-client (kein GTK), Cairo/Pango, eine Fläche je Ausgang, Uhr, Passwortfeld | `src/sperre.rs` |
| Passwort | PAM über `dlopen("libpam.so.0")`, Dienst `querencia-sitzung` falls in `/etc/pam.d`, sonst `login` | `src/pam.rs` |
| Leerlauf | ext-idle-notify-v1, startet `--sperren --sofort` | `src/leerlauf.rs` |
| polkit-Agent | Legitimierungsdialog für Aktionen mit `auth_admin` | `src/bin/querencia-polkit-agent.rs` |

`QS_TROCKENLAUF=1`: Abmelden, Neu starten und Herunterfahren werden nur
protokolliert. Zusammen mit `QS_TESTPASSWORT` und einem `XDG_RUNTIME_DIR`
außerhalb `/run/user/` entsperrt der Sperrbildschirm ohne PAM (nur in der
isolierten Prüfung möglich).

Bewusst **nicht** Teil dieses Programms: Benutzerwechsel, Ruhezustand,
Bildschirmschoner-Motive. Tastenkürzel (Strg+Alt+L, Strg+Alt+Entf) und den
Start der Leerlaufsperre richtet die Sitzung von Querencia Desktop ein, nicht
dieses Repo.

Anwendungskennung (App-ID): `org.querencia.Sitzung` (polkit-Agent:
`org.querencia.PolkitAgent`) · Binaries: `querencia-sitzung`,
`querencia-polkit-agent` · Debian-Paket: `querencia-sitzung`

## Stand

| | |
|---|---|
| Version | 0.1.3 (Tags `sitzung/0.1.3`, `sitzung/0.1.3-1`; Rückfragen ohne Countdown, sauberes Abmelden) |
| Letzte Prüfung | 20.10.2026: fmt, Clippy, 49 + 10 Unit-Tests; isolierte Regression 33 bestanden, 0 Fehler, 4 bewusst ausgelassen (echte logind/PAM/polkit), Sprache Deutsch und Englisch je 7 |
| Bekannte Lücken | siehe „Alpha“ |
| Getestet auf | Ubuntu 26.04, labwc 0.9.8, GTK 4.22, Rust 1.93 |
| Nicht getestet | andere Compositoren und Distributionen, HiDPI, echte Mehrmonitor-Hardware (nur zwei virtuelle Ausgänge) |

Änderungen: [CHANGELOG.md](CHANGELOG.md).

## Bauen

Voraussetzungen (Ubuntu 26.04):

```bash
sudo apt install build-essential pkg-config rustc cargo libgtk-4-dev libwayland-dev libxkbcommon-dev
```

```bash
cargo build --release --locked
```

Die Ergebnisse liegen unter `target/release/querencia-sitzung` und
`target/release/querencia-polkit-agent`. PAM wird zur Laufzeit geladen; es
braucht keine PAM-Entwicklungsdateien.

## Prüfen

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 werkzeug/i18n.py pruefen
```

Diese Befehle laufen ohne Bildschirm; die ersten drei sind auch die CI
(`.github/workflows/ci.yml`), die po-Prüfung braucht gettext und läuft lokal. Die **GUI-Prüfung** läuft in einer
abgeschotteten Sitzung (eigener labwc ohne Bildschirm, eigener D-Bus, eigenes
`XDG_RUNTIME_DIR` und Profil unter `gui-test/`, `QS_TROCKENLAUF=1`):

```bash
cargo build --locked
bash werkzeug/gui-pruefung.sh                        # alle Szenarien
bash werkzeug/gui-pruefung.sh szenario-dialog.sh     # ein Szenario
```

**Achtung:** Zwei Szenarien greifen bewusst auf das echte System zu:
`szenario-sperre.sh` fragt in S08 echtes PAM mit einem falschen Passwort ab,
`szenario-polkit.sh` meldet den Agenten für die echte logind-Sitzung bei
polkitd an. Beides nur auf einer Testmaschine laufen lassen. Ergebnisse und
Bilder landen unter `nachweis/gui/`; `qeingabe`/`qausgabe` werden aus
`werkzeug/qeingabe.c`, `werkzeug/qausgabe.c` und den Protokolldateien in
`werkzeug/protokolle/` gebaut (`gcc`, `wayland-client`, `xkbcommon`).

Arbeitsweise: [CONTRIBUTING.md](CONTRIBUTING.md).

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

## Vorbild und Abgrenzung

Vorbild für Bedienung und Aussehen sind **die Abmelde-, Herunterfahr- und
Sperrdialoge aus GNOME 2**. Querencia übernimmt davon die Bedienideen
(Dialogaufbau, Countdown, Kürzel), **keinen Quellcode, keine Texte, keine
Grafiken**. Das Projekt ist mit GNOME und dessen Autoren weder verbunden noch
von ihnen unterstützt.

Abweichungen vom Vorbild und Dinge, die bewusst fehlen:

- GTK 4 statt GTK 2, Fensterrahmen vom Compositor (labwc), kein libadwaita;
  der Sperrbildschirm nutzt ext-session-lock-v1 statt eines X-Bildschirmschoners.
- Kein Benutzerwechsel, kein Ruhezustand.

Fremdbestandteile: Rust-Abhängigkeiten (gtk4-rs, wayland-client,
wayland-protocols, xkbcommon, cairo-rs, pango und Umfeld), Liste mit
`cargo metadata --locked`; dynamisch gelinkt: GTK 4, libwayland, libxkbcommon,
zur Laufzeit libpam. In `werkzeug/protokolle/` liegen von `wayland-scanner`
erzeugte Protokolldateien für die Prüfwerkzeuge mit eigenem Vermerk:
`wlr-output-management-unstable-v1-*` (© 2019 Purism SPC, HPND-artige
Erlaubnis, siehe Dateikopf), `wlr-virtual-pointer-unstable-v1-*`
(© 2019 Josef Gajdusek, MIT, siehe Dateikopf) und
`virtual-keyboard-unstable-v1-*` (gekürzte Fassung des wlroots-Protokolls,
© 2008–2011 Kristian Høgsberg, © 2010–2013 Intel Corporation, © 2012–2013
Collabora, Ltd., © 2018 Purism SPC, MIT, siehe Dateikopf).

## Lizenz

Programm: GPL-3.0-or-later, siehe [LICENSE](LICENSE). Copyright 2026
Querencia Desktop contributors; Urheberhinweis in [COPYRIGHT](COPYRIGHT). Dateien mit eigenem Lizenzvermerk (die
Protokolldateien in `werkzeug/protokolle/`, siehe oben) behalten diesen.

Name und Logo „Querencia“ sind keine freien Inhalte: [MARKE.md](MARKE.md).

## Fehler melden

Fehler, Wünsche und Fragen zu Querencia gehen an die gemeinsame Meldestelle:
<https://github.com/querencia-desktop/querencia/issues/new/choose>.
Sicherheitslücken bitte nicht öffentlich melden, siehe [SECURITY.md](SECURITY.md).
