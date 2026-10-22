# Änderungen

## 0.1.7 – 2026-10-23

Dialog „Herunterfahren“ mit „Neu starten“:

- `--herunterfahren` (Panel → System → „Ausschalten …“) bietet wie in
  GNOME 2 auch „Neu starten“ an. Reihenfolge: Abbrechen, Neu starten,
  Herunterfahren. Den Fokus hat weiter „Abbrechen“; Herunterfahren ist der
  Vorgabeknopf und das Ziel eines Countdowns. „Neu starten“ löst sofort den
  Neustart aus, über denselben logind-Weg wie `--neustart`
  (`CanReboot`, dann `Reboot` mit interactive=true).
- Frage „Shut down or restart this system now?“ bzw. „Möchten Sie den
  Rechner jetzt herunterfahren oder neu starten?“. Die Zugriffstasten sind
  verschieden (de A/t/H, en C/R/S).
- Bereitschaft und Ruhezustand sind nicht dabei.
- Prüfwerkzeug: `szenario-herunterfahren.sh` (H01–H12) mit
  `logind-ersatz.py` auf einem Wegwerf-Systembus.

## 0.1.6 – 2026-10-22

Bildschirm aus nach der Sperre:

- Ist die Sitzung gesperrt und kommt `BildschirmAus` lang keine Eingabe,
  schaltet der Leerlaufdienst alle Ausgänge über
  wlr-output-power-management ab (labwc 0.9.8 bietet
  `zwlr_output_power_manager_v1`). Eine Eingabe (Maus, Taste) schaltet sie
  sofort wieder an, ebenso das Entsperren. Ohne weitere Eingabe gehen sie
  nach derselben Zeit wieder aus.
- Einstellung in `~/.config/querencia/sitzung.ini`: `[Sperre]`
  `BildschirmAus=` in Sekunden (`120`, `120s`) oder Minuten (`2min`);
  `0` = nie. Ohne Eintrag gelten 120 s.
- Gilt eine Hemmung über `org.freedesktop.ScreenSaver` (Video), bleiben die
  Ausgänge an; eine neue Hemmung schaltet sie an, fällt die letzte weg,
  zählt die Zeit von vorn. `SimulateUserActivity` schaltet sie ebenfalls
  an. Wayland-Hemmungen (idle-inhibit) wertet labwc selbst aus.
- Auch eine Sperre vom Tastenkürzel zählt; der Dienst bemerkt sie nach
  höchstens 5 s.
- Neue Abhängigkeit `wayland-protocols-wlr` 0.3 (MIT).
- Prüfwerkzeug: `szenario-bildschirm.sh` (B01–B15).

## 0.1.5 – 2026-10-22

Dunkelmodus für fremde GTK-3-Programme (QD-103):

- Der Leerlaufdienst schaltet bei `dunkel_aktiv` mit: Farbschema
  `prefer-dark` (gsettings `org.gnome.desktop.interface color-scheme`)
  und ein GTK-Thema mit `gtk-4.0/gtk-dark.css`, wie Panel und
  Einstellungen. Er gleicht beim Start ab und bei jedem Wechsel von
  Farbschema oder Thema.
- GTK 3.24 kennt das Farbschema des Portals nicht und liest
  `gtk-application-prefer-dark-theme` nur beim Start aus
  `~/.config/gtk-3.0/settings.ini` (isoliert gemessen,
  `nachweis/015-gtk3-messung`). Die Sitzung setzt dort einen
  gekennzeichneten Block mit `[Settings]` und
  `gtk-application-prefer-dark-theme=true`, nur wenn das Thema
  `gtk-3.0/gtk-dark.css` hat. Laufende GTK-3-Programme wechseln erst beim
  nächsten Start.
- Bei „hell“ fällt der Block wieder weg. Fremde Zeilen bleiben byte-gleich;
  eine Datei, die nur aus dem Block bestand, wird entfernt. Verweise
  (Symlinks) und Dateien mit beschädigtem Block bleiben unberührt, das
  Protokoll nennt sie.
- Setzt der Benutzer `gtk-application-prefer-dark-theme` in `[Settings]`
  selbst, gilt sein Eintrag: Die Sitzung schreibt dann keinen Block (kein
  zweiter Eintrag, der ihn überstimmt) und nennt das im Protokoll.
- Die labwc-Fensterrahmen schaltet die Sitzung nicht. Ein Entwurf mit
  `themerc-override` ist wieder ausgebaut (Entscheidung der Startsession,
  22.10.2026): Er konnte die Titelknöpfe (PNG des Themas) nicht tauschen.
  Ein dunkles labwc-Thema wählen die Einstellungen in `rc.xml`.
- Prüfwerkzeug: `szenario-gtk3-messung.sh` (Messung GTK 3, Fälle m1–m10),
  `szenario-fremd-dunkel.sh` (F01–F14), `gtk3-fenster.py` (GTK-3-Fenster
  über ctypes, ohne GObject-Introspection), `rahmenfarbe.py`
  (Titelleiste im Foto).

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
- Kästchen in der labwc-Titelleiste (QD-123): nachgestellt mit „4K #“ und
  einem Hashtag in Japanisch/Koreanisch oder in Vollbreite-Zeichen. Das
  System hat keine Schrift für CJK und für Vollbreite-Formen (nur DejaVu,
  FreeFont, Noto Color Emoji). labwc zeigt dafür Kästchen mit Hex-Ziffern.
  Emoji (auch Tag-Flaggen, Emoji 17, ZWJ, VS16) und Arabisch mit Emoji
  erscheinen richtig. Keine Änderung an `rc.xml`; Abhilfe sind weitere
  Schriften im System, Grenze in `docs/testing.md`.
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
