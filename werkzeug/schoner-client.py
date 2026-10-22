#!/usr/bin/env python3
"""Kleiner Testclient für org.freedesktop.ScreenSaver (QD-104), nur für die
isolierte Prüfung (szenario-schoner.sh). Hält seine Busverbindung offen,
damit Hemmungen gelten, bis er sie aufhebt oder endet.

Befehle zeilenweise auf stdin, Antworten zeilenweise auf stdout:
  inhibit ANWENDUNG GRUND [PFAD]  → "cookie N" oder "fehler TEXT"
  uninhibit N [PFAD]              → "ok" oder "fehler TEXT"
  name                            → eigener eindeutiger Busname
  ende                            → beendet den Client

PFAD ist der Objektpfad (Vorgabe /org/freedesktop/ScreenSaver; Firefox
nimmt /ScreenSaver).
"""

import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

NAME = "org.freedesktop.ScreenSaver"
PFAD = "/org/freedesktop/ScreenSaver"


def main() -> int:
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    for zeile in sys.stdin:
        teile = zeile.split()
        if not teile:
            continue
        try:
            if teile[0] == "inhibit":
                pfad = teile[3] if len(teile) > 3 else PFAD
                r = bus.call_sync(NAME, pfad, NAME, "Inhibit",
                                  GLib.Variant("(ss)", (teile[1], teile[2])),
                                  GLib.VariantType("(u)"), Gio.DBusCallFlags.NONE, 5000, None)
                print(f"cookie {r.unpack()[0]}", flush=True)
            elif teile[0] == "uninhibit":
                pfad = teile[2] if len(teile) > 2 else PFAD
                bus.call_sync(NAME, pfad, NAME, "UnInhibit",
                              GLib.Variant("(u)", (int(teile[1]),)),
                              None, Gio.DBusCallFlags.NONE, 5000, None)
                print("ok", flush=True)
            elif teile[0] == "name":
                print(bus.get_unique_name(), flush=True)
            elif teile[0] == "ende":
                return 0
            else:
                print(f"fehler unbekannter Befehl {teile[0]}", flush=True)
        except GLib.Error as e:
            print(f"fehler {e.message}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
