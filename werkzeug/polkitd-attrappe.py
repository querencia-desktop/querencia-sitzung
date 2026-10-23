#!/usr/bin/env python3
"""Querencia Sitzung 0.1.8: polkitd-Attrappe für den isolierten Lauf.

Belegt auf einem eigenen Wegwerf-Bus (Adresse als Argument, gestartet vom
Szenario mit dbus-daemon) den Namen org.freedesktop.PolicyKit1 und nimmt am
Objekt /org/freedesktop/PolicyKit1/Authority Registrierungen von
Legitimierungsagenten an, wie polkitd: je Subjekt ein Agent, ein zweiter
bekommt „An authentication agent already exists for the given subject“.
Jede Registrierung steht als „REGISTRIERT …“ mit Absender, Subjekt, Locale
und Objekt im Protokoll.

Steuerung über Signale:
  SIGUSR1  BeginAuthentication an den zuletzt registrierten Agenten (Cookie
           attrappe-<pid>-<n>, Identität unix-user --uid); die Antwort steht
           als „ANTWORT …“ im Protokoll
  SIGUSR2  Namen abgeben bzw. wieder nehmen (abwechselnd); beim Abgeben
           vergisst die Attrappe ihre Agenten wie ein neuer polkitd
  SIGTERM  beenden (der Name verschwindet mit der Verbindung)

Optionen für den Neustart-Fall:
  --verweigern N  die ersten N Registrierungen mit einem Fehler beantworten
  --schweigen N   danach N Registrierungen gar nicht beantworten (der Agent
                  läuft in seine Zeitgrenze)
  --uid UID       Identität der Prüfanfrage (Vorgabe 65534, nobody: ein
                  gesperrtes Systemkonto ohne Passwort, kein echtes Konto;
                  ein Konto, das es gar nicht gibt, lehnt libpolkit-agent-1
                  schon vor dem Passwortdialog ab)

Das echte polkitd wird nie gefragt; der Agent erreicht die Attrappe nur,
wenn sein DBUS_SYSTEM_BUS_ADDRESS auf den Wegwerf-Bus zeigt.

Aufruf (nur IN der isolierten Sitzung):
  polkitd-attrappe.py <bus-adresse> <protokoll> <bereit-datei> [Optionen]
"""

import argparse
import os
import signal
import sys

from gi.repository import Gio, GLib

NAME = "org.freedesktop.PolicyKit1"
PFAD = "/org/freedesktop/PolicyKit1/Authority"
XML = """
<node>
  <interface name="org.freedesktop.PolicyKit1.Authority">
    <method name="RegisterAuthenticationAgent">
      <arg type="(sa{sv})" name="subject" direction="in"/>
      <arg type="s" name="locale" direction="in"/>
      <arg type="s" name="object_path" direction="in"/>
    </method>
    <method name="UnregisterAuthenticationAgent">
      <arg type="(sa{sv})" name="subject" direction="in"/>
      <arg type="s" name="object_path" direction="in"/>
    </method>
  </interface>
</node>
"""


def subjekt_text(subjekt):
    art, werte = subjekt
    return art + ":" + ",".join(f"{k}={werte[k]}" for k in sorted(werte))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("adresse")
    ap.add_argument("protokoll")
    ap.add_argument("bereit")
    ap.add_argument("--verweigern", type=int, default=0)
    ap.add_argument("--schweigen", type=int, default=0)
    ap.add_argument("--uid", type=int, default=65534)
    a = ap.parse_args()
    log = open(a.protokoll, "a", buffering=1)

    def schreiben(text):
        jetzt = GLib.get_monotonic_time() // 1000
        log.write(f"{jetzt} [{os.getpid()}] {text}\n")

    bus = Gio.DBusConnection.new_for_address_sync(
        a.adresse,
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT
        | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
        None,
        None,
    )
    zustand = {"aufrufe": 0, "anfragen": 0, "besitzt": False}
    agenten = {}  # Subjekt-Text -> (Absender, Objekt)
    letzter = []  # zuletzt registrierter Agent
    haengend = []  # unbeantwortete Aufrufe (--schweigen)

    def aufruf(_bus, sender, _pfad, _iface, methode, parameter, antwort):
        if methode == "UnregisterAuthenticationAgent":
            subjekt, _objekt = parameter.unpack()
            agenten.pop(subjekt_text(subjekt), None)
            schreiben(f"ABGEMELDET {sender} subjekt={subjekt_text(subjekt)}")
            antwort.return_value(None)
            return
        zustand["aufrufe"] += 1
        n = zustand["aufrufe"]
        subjekt, locale, objekt = parameter.unpack()
        s = subjekt_text(subjekt)
        if n <= a.verweigern:
            schreiben(f"VERWEIGERT #{n} {sender} subjekt={s}")
            antwort.return_dbus_error(
                "org.freedesktop.PolicyKit1.Error.Failed", "Attrappe: noch nicht bereit"
            )
            return
        if n <= a.verweigern + a.schweigen:
            schreiben(f"SCHWEIGT #{n} {sender} subjekt={s}")
            haengend.append(antwort)
            return
        if s in agenten:
            schreiben(f"DOPPELT #{n} {sender} subjekt={s}")
            antwort.return_dbus_error(
                "org.freedesktop.PolicyKit1.Error.Failed",
                "An authentication agent already exists for the given subject",
            )
            return
        agenten[s] = (sender, objekt)
        letzter[:] = [(sender, objekt)]
        schreiben(f"REGISTRIERT #{n} {sender} subjekt={s} locale={locale} objekt={objekt}")
        antwort.return_value(None)

    info = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]
    bus.register_object(PFAD, info, aufruf, None, None)

    def name_nehmen():
        ergebnis = bus.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
            "RequestName", GLib.Variant("(su)", (NAME, 4)),
            GLib.VariantType("(u)"), Gio.DBusCallFlags.NONE, 5000, None,
        ).unpack()[0]
        zustand["besitzt"] = ergebnis == 1
        schreiben(f"GENOMMEN {NAME} als {bus.get_unique_name()} (RequestName {ergebnis})")
        return ergebnis == 1

    def name_abgeben():
        ergebnis = bus.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
            "ReleaseName", GLib.Variant("(s)", (NAME,)),
            GLib.VariantType("(u)"), Gio.DBusCallFlags.NONE, 5000, None,
        ).unpack()[0]
        zustand["besitzt"] = False
        agenten.clear()
        letzter.clear()
        schreiben(f"ABGEGEBEN {NAME} (ReleaseName {ergebnis}), Agenten vergessen")

    def anfrage():
        if not letzter:
            schreiben("ANFRAGE ohne registrierten Agenten")
            return GLib.SOURCE_CONTINUE
        zustand["anfragen"] += 1
        cookie = f"attrappe-{os.getpid()}-{zustand['anfragen']}"
        sender, objekt = letzter[0]
        schreiben(f"ANFRAGE {cookie} an {sender} {objekt} (uid {a.uid})")
        parameter = GLib.Variant(
            "(sssa{ss}sa(sa{sv}))",
            (
                "org.querencia.attrappe.pruefen",
                "Attrappe: Prüfanfrage nach dem Neustart",
                "",
                {},
                cookie,
                [("unix-user", {"uid": GLib.Variant("u", a.uid)})],
            ),
        )

        def fertig(verbindung, ergebnis):
            try:
                verbindung.call_finish(ergebnis)
                schreiben(f"ANTWORT {cookie} erfolgreich")
            except GLib.Error as e:
                schreiben(f"ANTWORT {cookie} Fehler {Gio.DBusError.get_remote_error(e)}: {e.message}")

        bus.call(
            sender, objekt, "org.freedesktop.PolicyKit1.AuthenticationAgent",
            "BeginAuthentication", parameter, None, Gio.DBusCallFlags.NONE,
            120000, None, fertig,
        )
        return GLib.SOURCE_CONTINUE

    def umschalten():
        if zustand["besitzt"]:
            name_abgeben()
        else:
            name_nehmen()
        return GLib.SOURCE_CONTINUE

    if not name_nehmen():
        schreiben("FEHLER Name nicht bekommen")
        return 1
    schreiben(f"BEREIT {a.adresse} verweigern={a.verweigern} schweigen={a.schweigen}")
    open(a.bereit, "w").close()
    schleife = GLib.MainLoop()
    GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGUSR1, anfrage)
    GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGUSR2, umschalten)
    GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGTERM, lambda: schleife.quit() or GLib.SOURCE_REMOVE)
    bus.connect("closed", lambda *_: schleife.quit())
    schleife.run()
    schreiben("ENDE")
    return 0


if __name__ == "__main__":
    sys.exit(main())
