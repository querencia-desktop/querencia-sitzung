# Änderungen

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
