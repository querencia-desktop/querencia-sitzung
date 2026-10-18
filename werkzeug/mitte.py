#!/usr/bin/env python3 -I
"""Misst im Bildschirmfoto (PNG, dunkler Hintergrund) das helle Fenster und
prüft, ob seine Mitte in der Bildmitte liegt (±12 px).
   mitte.py BILD.png   → Ausgabe und Exit 0 (mittig) / 1 (nicht mittig)"""
import struct, sys, zlib

f = open(sys.argv[1], 'rb').read()
pos = 8; w = h = ct = 0; data = b''
while pos < len(f):
    l = struct.unpack('>I', f[pos:pos+4])[0]; t = f[pos+4:pos+8]; c = f[pos+8:pos+8+l]
    if t == b'IHDR': w, h, bd, ct = struct.unpack('>IIBB', c[:10])
    if t == b'IDAT': data += c
    pos += 12 + l
raw = zlib.decompress(data); bpp = 4 if ct == 6 else 3; stride = w * bpp
rows = []; prev = bytearray(stride); p = 0
for y in range(h):
    ft = raw[p]; p += 1; line = bytearray(raw[p:p+stride]); p += stride
    if ft:
        for i in range(stride):
            a = line[i-bpp] if i >= bpp else 0; b = prev[i]; c = prev[i-bpp] if i >= bpp else 0
            if ft == 1: line[i] = (line[i] + a) & 255
            elif ft == 2: line[i] = (line[i] + b) & 255
            elif ft == 3: line[i] = (line[i] + (a + b) // 2) & 255
            elif ft == 4:
                pa = abs(b-c); pb = abs(a-c); pc = abs(a+b-2*c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 255
    rows.append(bytes(line)); prev = line
def hell(x, y):
    r, g, b = rows[y][x*bpp:x*bpp+3]; return r + g + b > 150
xs = [x for x in range(w) if any(hell(x, y) for y in range(0, h, 4))]
ys = [y for y in range(h) if any(hell(x, y) for x in range(0, w, 4))]
if not xs or not ys:
    print("kein Fenster gefunden"); sys.exit(1)
mx, my = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
ok = abs(mx - w / 2) <= 12 and abs(my - h / 2) <= 12
print(f"Bild {w}x{h}, Fenster {min(xs)}..{max(xs)} x {min(ys)}..{max(ys)} "
      f"({max(xs)-min(xs)+1}x{max(ys)-min(ys)+1}), Mitte ({mx:.0f},{my:.0f}) "
      f"soll ({w/2:.0f},{h/2:.0f}): {'mittig' if ok else 'NICHT mittig'}")
sys.exit(0 if ok else 1)
