# Änderungen

## 0.1.4 – 2026-10-22

Bildschirmschoner-Dienst, Dunkelmodus der Dialoge (QD-104, QD-109, QD-123):

- Der Leerlaufdienst bietet `org.freedesktop.ScreenSaver` auf dem
  Sitzungsbus an (an `/org/freedesktop/ScreenSaver` und `/ScreenSaver`).
  Browser und Player halten damit die Leerlaufsperre an, direkt oder über
  `xdg-desktop-portal-gtk`. Das Portal meldete bisher etwa alle 10 s
  „Backend call failed“.
  - `Inhibit`/`UnInhibit`: Solange ein Cookie gilt, sperrt der Leerlauf
    nicht. Fällt das letzte weg, zählt der Leerlauf von vorn. Ein Cookie
    hebt nur auf, wer es geholt hat. Verlässt ein Programm den Bus,
    fallen seine Cookies weg. Je Programm höchstens 64.
  - `Lock` und `SetActive(true)` sperren sofort. `SetActive(false)`
    entsperrt nicht und liefert `false`.
  - `GetActive`, `GetActiveTime` und das Signal `ActiveChanged` folgen dem
    Sperrbildschirm, auch dem vom Tastenkürzel (der wird spätestens nach
    5 s bemerkt).
  - `SimulateUserActivity` zählt den Leerlauf von vorn.
  - `org.gnome.SessionManager` bietet die Sitzung bewusst nicht an. Mit
    einem Besitzer dieses Namens schaltet das Portal auf den GNOME-Weg um
    und erwartet die ganze Sitzungsverwaltung von gnome-session.
  - Der Leerlaufdienst läuft dafür in einer GLib-Hauptschleife
    (neue Abhängigkeit `glib-unix`). Einen eigenen Autostart braucht der
    Dienst nicht, er kommt mit dem Leerlaufdienst.
- Dialog „Sitzung beenden“, Rückfragen und polkit-Agent folgen jetzt auch
  dann dem dunklen Farbschema, wenn es beim Start schon gilt. GTK 4.22
  zeigte sie in diesem Fall hell, bis das Schema wechselte. Die Dialoge
  melden die Einstellung `gtk-interface-color-scheme` nach dem Start
  einmal neu (wie das Panel). Eigene Farben haben sie weiterhin nicht.
- Emoji in der labwc-Titelleiste (QD-123): Isoliert erscheinen einfache
  Emoji, Flaggen, Flaggen mit Tag-Folgen, ZWJ-Folgen, Hauttöne und Emoji
  17 richtig. Tag-Zeichen ohne Flagge bleiben unsichtbar. Kästchen mit
  Hex-Ziffern zeigt labwc nur für Zeichen, die keine installierte Schrift
  hat (das System hat nur DejaVu, FreeFont und Noto Color Emoji). Keine
  Änderung an `rc.xml`, Grenze in `docs/testing.md`.
- Prüfwerkzeug: Szenarien `szenario-schoner.sh`, `szenario-dunkel.sh`
  (Portal-Ersatz aus der Dunkel-Prüfung von Screenshot 0.2.10) und
  `szenario-emoji.sh`; `isoliert.py` kennt `QS_LAUFZEIT`, `QS_PORTAL`,
  `QS_RC_XML` und `QS_LANG`.

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
- Doku nach ENTSCHEIDUNGEN Nr. 17 (`docs/building.md`, `docs/testing.md`,
  Arbeitsregeln), Lizenzkopf (Nr. 14 Q5), dieses
  Änderungsprotokoll; Verweise ins Leere (GH-3) in `isoliert.py` ersetzt.

## 0.1.1-2 – 2026-10-19

Eigenes Programmsymbol (G2-V, QD-034) statt Logo-Platzhalter; Programme
unverändert.

## 0.1.1 – 2026-10-18

`querencia-polkit-agent` (polkit-Legitimierungsagent der Sitzung, G2-K),
Integration und Prüfung erweitert; Menüeintrag mit absoluten Pfaden (0.1.1-1).

## 0.1.0 – 2026-10-18

Erster Stand (G2-K): Dialog „Sitzung beenden“, Rückfragen mit Countdown,
Sperrbildschirm über ext-session-lock mit PAM, Leerlaufsperre.
