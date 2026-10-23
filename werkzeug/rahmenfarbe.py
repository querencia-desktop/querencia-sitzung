#!/usr/bin/env python3
"""Farbe der labwc-Titelleiste im Bildschirmfoto (0.1.5, QD-088 Punkt 6).

   rahmenfarbe.py BILD.png

Das Fenster steht mittig (labwc-Platzierung „center“) vor dem schwarzen
Grund des headless labwc. Von oben die Bildmitte entlang: der erste Punkt,
der sich vom Grund abhebt, ist der Rahmen; die Zeilen 3 bis 20 darunter sind
die Titelleiste. In einem Streifen von 120 px links bis 80 px rechts der
Mitte (Titeltext, ohne die Knöpfe rechts): häufigste Farbe =
Titelgrund, häufigste deutlich andere Farbe = Titelschrift; dazu Kontrast
nach WCAG 2. Ausgabe wie grundfarbe.py."""
import collections
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf  # noqa: E402


def leuchtdichte(c):
    w = []
    for x in c:
        x /= 255
        w.append(x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4)
    return 0.2126 * w[0] + 0.7152 * w[1] + 0.0722 * w[2]


def main():
    pb = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1])
    b, h, n, rs = pb.get_width(), pb.get_height(), pb.get_n_channels(), pb.get_rowstride()
    px = pb.get_pixels()

    def farbe(x, y):
        o = y * rs + x * n
        return tuple(px[o:o + 3])

    grund = farbe(2, 2)
    mx = b // 2
    oben = next((y for y in range(h) if sum(abs(a - g) for a, g in zip(farbe(mx, y), grund)) > 24), None)
    if oben is None:
        print("kein Fenster gefunden")
        return 1
    zaehler = collections.Counter(
        farbe(x, y) for y in range(oben + 3, min(h, oben + 21)) for x in range(max(0, mx - 120), min(b, mx + 80)))
    titel, anzahl = zaehler.most_common(1)[0]
    lt = leuchtdichte(titel)
    schrift = next((c for c, _ in zaehler.most_common() if abs(leuchtdichte(c) - lt) > 0.3), titel)
    ls = leuchtdichte(schrift)
    kontrast = (max(lt, ls) + 0.05) / (min(lt, ls) + 0.05)
    hexf = "#%02x%02x%02x"
    print(f"Titel {hexf % titel} ({100 * anzahl // sum(zaehler.values())} %), Rahmen oben y={oben} {hexf % farbe(mx, oben)}, "
          f"Leuchtdichte {lt:.3f} {'hell' if lt > 0.4 else 'dunkel'}, Schrift {hexf % schrift}, Kontrast {kontrast:.1f}:1")
    return 0


if __name__ == "__main__":
    sys.exit(main())
