#!/usr/bin/env python3
"""Grundfarbe und Schriftfarbe eines Dialogs im Bildschirmfoto (QD-109).

   grundfarbe.py BILD.png [BREITE HOEHE]

Untersucht ein Rechteck der Größe BREITE×HOEHE (Vorgabe 300×80) um die
Bildmitte, wo die Dialoge der Sitzung stehen (labwc-Regel AutoPlace center):
häufigste Farbe = Grund; häufigste Farbe unter den Bildpunkten, deren
Leuchtdichte deutlich anders ist = Schrift. Gibt beide, die Leuchtdichte des
Grunds, „hell“/„dunkel“ und den Kontrast nach WCAG 2 aus."""
import collections
import struct
import sys
import zlib


def lesen(pfad):
    f = open(pfad, "rb").read()
    pos = 8
    w = h = ct = 0
    data = b""
    while pos < len(f):
        laenge = struct.unpack(">I", f[pos:pos + 4])[0]
        typ = f[pos + 4:pos + 8]
        inhalt = f[pos + 8:pos + 8 + laenge]
        if typ == b"IHDR":
            w, h, _, ct = struct.unpack(">IIBB", inhalt[:10])
        if typ == b"IDAT":
            data += inhalt
        pos += 12 + laenge
    raw = zlib.decompress(data)
    bpp = 4 if ct == 6 else 3
    stride = w * bpp
    zeilen = []
    prev = bytearray(stride)
    p = 0
    for _ in range(h):
        ft = raw[p]
        p += 1
        line = bytearray(raw[p:p + stride])
        p += stride
        if ft:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                if ft == 1:
                    line[i] = (line[i] + a) & 255
                elif ft == 2:
                    line[i] = (line[i] + b) & 255
                elif ft == 3:
                    line[i] = (line[i] + (a + b) // 2) & 255
                elif ft == 4:
                    pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                    line[i] = (line[i] + (a if pa <= pb and pa <= pc else (b if pb <= pc else c))) & 255
        zeilen.append(bytes(line))
        prev = line
    return w, h, bpp, zeilen


def leuchtdichte(rgb):
    def kanal(c):
        c /= 255
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (kanal(c) for c in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def main():
    w, h, bpp, zeilen = lesen(sys.argv[1])
    bw = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    bh = int(sys.argv[3]) if len(sys.argv) > 3 else 80
    x0, y0 = max(0, w // 2 - bw // 2), max(0, h // 2 - bh // 2)
    zaehler = collections.Counter()
    for y in range(y0, min(h, y0 + bh)):
        z = zeilen[y]
        for x in range(x0, min(w, x0 + bw)):
            zaehler[tuple(z[x * bpp:x * bpp + 3])] += 1
    grund, n = zaehler.most_common(1)[0]
    lg = leuchtdichte(grund)
    andere = collections.Counter({f: k for f, k in zaehler.items() if abs(leuchtdichte(f) - lg) > 0.3})
    schrift = andere.most_common(1)[0][0] if andere else grund
    ls = leuchtdichte(schrift)
    kontrast = (max(lg, ls) + 0.05) / (min(lg, ls) + 0.05)
    print(f"Grund #{bytes(grund).hex()} ({100 * n / sum(zaehler.values()):.0f} %), "
          f"Leuchtdichte {lg:.3f} {'dunkel' if lg < 0.2 else 'hell'}, "
          f"Schrift #{bytes(schrift).hex()}, Kontrast {kontrast:.1f}:1")


if __name__ == "__main__":
    main()
