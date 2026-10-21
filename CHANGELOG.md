# Änderungen

## 0.1.3 – 2026-10-20

Sauberes Abmelden, Rückfragen ohne Countdown (QD-062, G2-AI):

- Abmelden, Neu starten, Herunterfahren und der Gesamtdialog zählen nicht
  mehr von selbst herunter. Sie warten, bis jemand wählt (Entscheidung des
  Besitzers, 20.10.2026: „lass countdown weg“). „Abbrechen“ bleibt der
  Standardknopf für Esc und Enter. Am 20.10. hatte ein stehengelassener
  Dialog hinter einem anderen Fenster nach 60 s abgemeldet.
- Den Countdown gibt es nur noch auf Wunsch: `--countdown SEK` oder
  `~/.config/querencia/sitzung.ini` mit `[Dialog]` und `Countdown=an`
  (60 s). Dann zählt er nur, solange der Dialog aktiv ist, zeigt sonst
  „Angehalten, solange dieses Fenster nicht aktiv ist.“ und bricht nach
  60 s am Stück ohne Fokus ab. `--kein-countdown` entfällt.
- Abmelden beendet vor `labwc --exit` die Programme der Sitzung: Panel,
  Arbeitsfläche, Benachrichtigungen, polkit-Agent, Leerlaufdienst und die
  Applet-Helfer. Erkannt werden sie an `LABWC_PID` oder an den
  PID-Dateien des Starters. Ablauf: SIGTERM, 2 s Frist, dann SIGKILL.
  Danach stoppt es `xdg-desktop-portal(-gtk)` per `systemctl --user`
  (`reset-failed` vorher und nachher). Reihenfolge: Programme, Portale,
  `labwc --exit`. Programme anderer Sitzungen und vom Benutzer gestartete
  Programme bleiben unberührt. Im Trockenlauf wird nur protokolliert.

## 0.1.2 – 2026-10-20

Englisch und Deutsch über GNU gettext, Kindprozesse geerntet (G2-AH):

- Oberflächentexte englisch im Code (Wortschatz gnome-session 2.30 und
  polkit-gnome), Deutsch aus `po/de.po` mit dem bisherigen Wortlaut:
  Dialog „Sitzung beenden“, Rückfragen, Countdown (Plural), Fehlermeldungen
  der Aktionen, polkit-Agent, Menüeintrag aus `.desktop.in`. Beide Programme
  teilen `querencia-sitzung.mo`. Sperrbildschirm und PAM-Meldungen bleiben
  vorerst deutsch.
- `i18n::init()` setzt die Locale am Programmanfang (Dialogtexte entstehen
  vor dem GTK-Start).
- QD-050: Der Leerlaufdienst erntet jeden Sperrprozess sofort in einem
  Warte-Thread. Bisher blieb nach dem Entsperren ein Zombie
  `[querencia-sitzu] <defunct>` bis zum nächsten Leerlauf stehen.
- GUI-Prüfung: `QS_SPRACHE`, `QS_GROESSE`, neues `szenario-sprache.sh`;
  `QS_OHNE_SYSTEM=1` (Vorgabe) lässt Fälle mit echtem logind/PAM/polkit aus.
- Doku unter `docs/` (`docs/building.md`, `docs/testing.md`), Lizenzkopf,
  dieses Änderungsprotokoll; tote Verweise in `isoliert.py` ersetzt.

## 0.1.1-2 – 2026-10-19

Eigenes Programmsymbol (G2-V, QD-034) statt Logo-Platzhalter; Programme
unverändert.

## 0.1.1 – 2026-10-18

`querencia-polkit-agent` (polkit-Legitimierungsagent der Sitzung, G2-K),
Integration und Prüfung erweitert; Menüeintrag mit absoluten Pfaden (0.1.1-1).

## 0.1.0 – 2026-10-18

Erster Stand (G2-K): Dialog „Sitzung beenden“, Rückfragen mit Countdown,
Sperrbildschirm über ext-session-lock mit PAM, Leerlaufsperre.
