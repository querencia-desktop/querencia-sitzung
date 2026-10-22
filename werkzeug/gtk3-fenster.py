#!/usr/bin/env python3
"""Querencia Sitzung 0.1.5 (QD-103): ein fremdes GTK-3-Programm für die
isolierte Prüfung. Auf der Entwicklungs-VM gibt es weder gtk3-demo noch die
GObject-Introspection von GTK 3; das Fenster entsteht deshalb über ctypes
direkt aus libgtk-3.so.0 (GTK 3.24, Wayland-Backend).

Zeigt ein Fenster mit Titel und Beschriftung und schreibt nach dem Start und
bei jeder Änderung eine Zeile:
  t=<s> thema=<gtk-theme-name> dunkel=<gtk-application-prefer-dark-theme>
        grund=<#rrggbb theme_bg_color> schrift=<#rrggbb theme_fg_color>

Aufruf (nur IN der isolierten Sitzung):  gtk3-fenster.py <titel> <sekunden>
"""

import ctypes
import sys
import time

gtk = ctypes.CDLL("libgtk-3.so.0")
gobj = ctypes.CDLL("libgobject-2.0.so.0")
glib = ctypes.CDLL("libglib-2.0.so.0")


class RGBA(ctypes.Structure):
    _fields_ = [("r", ctypes.c_double), ("g", ctypes.c_double),
                ("b", ctypes.c_double), ("a", ctypes.c_double)]


P = ctypes.c_void_p
gtk.gtk_init_check.restype = ctypes.c_int
gtk.gtk_window_new.restype = P
gtk.gtk_label_new.restype = P
gtk.gtk_label_new.argtypes = [ctypes.c_char_p]
gtk.gtk_settings_get_default.restype = P
gtk.gtk_widget_get_style_context.restype = P
gtk.gtk_widget_get_style_context.argtypes = [P]
gtk.gtk_style_context_lookup_color.argtypes = [P, ctypes.c_char_p, ctypes.POINTER(RGBA)]
gtk.gtk_window_set_title.argtypes = [P, ctypes.c_char_p]
gtk.gtk_window_set_default_size.argtypes = [P, ctypes.c_int, ctypes.c_int]
gtk.gtk_container_add.argtypes = [P, P]
gtk.gtk_widget_show_all.argtypes = [P]
gobj.g_object_get.argtypes = [P, ctypes.c_char_p, P, P]
glib.g_main_context_iteration.argtypes = [P, ctypes.c_int]
glib.g_free.argtypes = [P]


def hex_farbe(c: RGBA) -> str:
    return "#%02x%02x%02x" % tuple(round(max(0.0, min(1.0, x)) * 255) for x in (c.r, c.g, c.b))


def zustand(settings, fenster) -> str:
    name = ctypes.c_char_p()
    gobj.g_object_get(settings, b"gtk-theme-name", ctypes.byref(name), None)
    dunkel = ctypes.c_int()
    gobj.g_object_get(settings, b"gtk-application-prefer-dark-theme", ctypes.byref(dunkel), None)
    ctx = gtk.gtk_widget_get_style_context(fenster)
    grund, schrift = RGBA(), RGBA()
    gtk.gtk_style_context_lookup_color(ctx, b"theme_bg_color", ctypes.byref(grund))
    gtk.gtk_style_context_lookup_color(ctx, b"theme_fg_color", ctypes.byref(schrift))
    return (f"thema={(name.value or b'').decode()} dunkel={dunkel.value} "
            f"grund={hex_farbe(grund)} schrift={hex_farbe(schrift)}")


def main() -> int:
    titel, sekunden = sys.argv[1], float(sys.argv[2])
    if not gtk.gtk_init_check(None, None):
        print("FEHLER gtk_init_check", flush=True)
        return 1
    fenster = gtk.gtk_window_new(0)
    gtk.gtk_window_set_title(fenster, titel.encode())
    gtk.gtk_window_set_default_size(fenster, 420, 240)
    gtk.gtk_container_add(fenster, gtk.gtk_label_new(b"GTK 3 - Querencia QD-103"))
    gtk.gtk_widget_show_all(fenster)
    settings = gtk.gtk_settings_get_default()
    anfang = time.monotonic()
    vorher = None
    while time.monotonic() - anfang < sekunden:
        while glib.g_main_context_iteration(None, 0):
            pass
        jetzt = zustand(settings, fenster)
        if jetzt != vorher:
            print(f"t={time.monotonic() - anfang:.1f} {jetzt}", flush=True)
            vorher = jetzt
        time.sleep(0.05)
    return 0


if __name__ == "__main__":
    sys.exit(main())
