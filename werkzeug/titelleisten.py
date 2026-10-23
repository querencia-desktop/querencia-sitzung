#!/usr/bin/env python3
"""Schneidet aus den Fotos von szenario-emoji.sh die Titelleiste (Fenster
mittig, AutoPlace) und stapelt sie zu einem Übersichtsbild (QD-123).
   titelleisten.py ZIEL.png BILD.png …"""
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf  # noqa: E402

X, Y, B, H = 425, 318, 430, 30
teile = [GdkPixbuf.Pixbuf.new_from_file(p).new_subpixbuf(X, Y, B, H) for p in sys.argv[2:]]
ziel = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, False, 8, B, H * len(teile))
for i, t in enumerate(teile):
    t.copy_area(0, 0, B, H, ziel, 0, i * H)
ziel.savev(sys.argv[1], "png", [], [])
