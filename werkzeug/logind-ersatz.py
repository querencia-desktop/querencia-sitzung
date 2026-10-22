#!/usr/bin/env python3
"""Querencia Sitzung 0.1.7: Ersatz für systemd-logind im isolierten Lauf.

Belegt auf einem eigenen Wegwerf-Bus (Adresse als Argument, gestartet vom
Szenario mit dbus-daemon) den Namen org.freedesktop.login1 und beantwortet
am Objekt /org/freedesktop/login1 (org.freedesktop.login1.Manager)
CanReboot und CanPowerOff mit „yes“. Reboot und PowerOff tun nichts außer
einer Zeile im Protokoll; im Trockenlauf dürfen sie nie ankommen. Jeder
Aufruf steht als „AUFRUF <Methode> <Argumente>“ in der Protokolldatei.

Die Sitzung erreicht den Ersatz nur, wenn ihr DBUS_SYSTEM_BUS_ADDRESS auf
den Wegwerf-Bus zeigt; das echte logind wird nie gefragt.

Aufruf (nur IN der isolierten Sitzung):
  logind-ersatz.py <bus-adresse> <protokoll> <bereit-datei>
"""

import sys

from gi.repository import Gio, GLib

PFAD = "/org/freedesktop/login1"
XML = """
<node>
  <interface name="org.freedesktop.login1.Manager">
    <method name="CanReboot"><arg type="s" direction="out"/></method>
    <method name="CanPowerOff"><arg type="s" direction="out"/></method>
    <method name="Reboot"><arg type="b" direction="in"/></method>
    <method name="PowerOff"><arg type="b" direction="in"/></method>
  </interface>
</node>
"""


def main():
    adresse, protokoll, bereit = sys.argv[1:4]
    log = open(protokoll, "a", buffering=1)
    bus = Gio.DBusConnection.new_for_address_sync(
        adresse,
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT
        | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
        None,
        None,
    )

    def aufruf(_bus, sender, _pfad, _iface, methode, parameter, antwort):
        log.write(f"AUFRUF {methode} {parameter.unpack()} von {sender}\n")
        if methode in ("CanReboot", "CanPowerOff"):
            antwort.return_value(GLib.Variant("(s)", ("yes",)))
        else:
            antwort.return_value(None)

    info = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]
    bus.register_object(PFAD, info, aufruf, None, None)
    ergebnis = bus.call_sync(
        "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
        "RequestName", GLib.Variant("(su)", ("org.freedesktop.login1", 4)),
        GLib.VariantType("(u)"), Gio.DBusCallFlags.NONE, 5000, None,
    )
    if ergebnis.unpack()[0] != 1:
        log.write(f"FEHLER RequestName: {ergebnis.unpack()}\n")
        return 1
    log.write(f"BEREIT {adresse}\n")
    open(bereit, "w").close()
    schleife = GLib.MainLoop()
    bus.connect("closed", lambda *_: schleife.quit())
    schleife.run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
