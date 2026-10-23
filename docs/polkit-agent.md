# polkit-Agent

`querencia-polkit-agent` ist der Legitimierungsagent der Querencia-Sitzung
(G2-K). Der Autostart der Sitzung startet ihn; er läuft bis zum Ende der
Sitzung und zeigt bei Bedarf den Passwortdialog.

## Registrierung

- Beim Start meldet der Agent am System-Bus das Objekt
  `/org/querencia/PolkitAgent` (`org.freedesktop.PolicyKit1.AuthenticationAgent`)
  an und ruft `RegisterAuthenticationAgent` bei polkitd.
- Subjekt: die logind-Sitzung aus `XDG_SESSION_ID`, sonst die Sitzung des
  Prozesses laut logind (`GetSessionByPID`), sonst der Prozess selbst.
- Locale: zuerst `LANGUAGE` (ein nackter Sprachcode wird zu `de_DE.UTF-8`),
  sonst `LC_ALL`, `LC_MESSAGES`, `LANG`.
- Die Passwortprüfung übernimmt `PolkitAgentSession` aus libpolkit-agent-1
  mit dem Helfer `polkit-agent-helper-1`; der Agent prüft nie selbst gegen
  PAM.

## Neustart von polkitd (0.1.8, QD-156)

Startet polkitd neu (z. B. needrestart nach einem Update), kennt der neue
polkitd keinen Agenten. Anfragen scheiterten dann sofort mit „failed to
obtain auth“, ohne Dialog.

Seit 0.1.8 beobachtet der Agent auf dem System-Bus `NameOwnerChanged` für
`org.freedesktop.PolicyKit1`. Das Abo steht vor der ersten Registrierung,
damit kein Wechsel dazwischen verloren geht. Der Agent merkt sich den
eindeutigen Busnamen des polkitd, bei dem er registriert ist.

| Ereignis | Verhalten | Protokoll |
|---|---|---|
| Name verschwindet | offene Anfrage verwerfen (der neue polkitd kennt ihr Cookie nicht), warten | `polkitd beendet (Besitzer :1.x), warte auf den neuen` |
| neuer Besitzer | `RegisterAuthenticationAgent` mit demselben Subjekt und derselben Locale wie beim Start | `neuer polkitd :1.y (vorher …), registriere erneut` |
| Registrierung gelungen | Besitzer merken | `polkitd neu gestartet, erneut registriert (polkitd :1.y, Versuch n, Subjekt …, Locale …, Objekt …)` |
| Fehler oder keine Antwort in 5 s | Wiederholung nach 0,5 – 1 – 2 – 4 – 8 – 15 s, dann alle 30 s; nach 12 Versuchen (3 min Wartezeit) Ruhe bis zum nächsten Besitzerwechsel | `polkitd :1.y antwortet noch nicht (Versuch n: …), nächster Versuch in … ms` |
| „already exists“ | ein Agent für das Subjekt ist schon registriert: keine Wiederholung | `polkitd :1.y kennt schon einen Agenten für …` |

Gibt derselbe Prozess den Namen ab und nimmt ihn wieder, gilt das ebenso
als neuer polkitd. Ein Wechsel ohne Lücke (alter und neuer Besitzer in einem
Signal) verwirft die offene Anfrage und registriert neu.

Der Agent startet polkitd nicht selbst: die erneute Registrierung läuft mit
`NO_AUTO_START`. Endet polkitd und startet niemand ihn neu, aktiviert ihn
erst die nächste Anfrage eines Programms (D-Bus-Aktivierung); der Agent
registriert sich, sobald der Name erscheint.

## Prüfung

`werkzeug/szenario-polkit-neustart.sh` (N01–N09) läuft vollständig isoliert:
eigener Wegwerf-Systembus mit `werkzeug/polkitd-attrappe.py`, Subjekt
`XDG_SESSION_ID=q1`; das echte polkitd und die echte logind-Sitzung werden
nie gefragt. Aufruf und Fälle: [testing.md](testing.md).
