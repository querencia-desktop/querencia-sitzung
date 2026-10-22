#!/usr/bin/env python3
"""Testtitel für die labwc-Titelleiste (QD-123), je Zeile „name<TAB>titel“.
Die Tag-Zeichen (U+E0020–U+E007F) stehen hier als Escape, damit die Quelle
frei von unsichtbaren Zeichen bleibt."""


def tags(text: str) -> str:
    return "".join(chr(0xE0000 + ord(c)) for c in text)


SCHWARZ = "\U0001F3F4"
ENDE = "\U000E007F"
TITEL = [
    ("t1-ascii", "Querencia Testfenster"),
    ("t2-einfach", "Einfach \U0001F600 ✅ ❤\ufe0f Text"),
    ("t3-flaggen", "Flaggen \U0001F1E9\U0001F1EA \U0001F1FA\U0001F1E6 \U0001F1EF\U0001F1F5"),
    ("t4-tags", f"England {SCHWARZ}{tags('gbeng')}{ENDE} Schottland {SCHWARZ}{tags('gbsct')}{ENDE} Wales {SCHWARZ}{tags('gbwls')}{ENDE}"),
    ("t5-zwj", "ZWJ \U0001F469\u200d\U0001F4BB \U0001F3F3\ufe0f\u200d\U0001F308"),
    ("t6-nur-tags", f"Nur Tags A{tags('hidden')}B"),
    ("t8-neu", "Emoji 16/17 \U0001FAE9 \U0001FAC6 \U0001FABE \U0001FAEA \U0001FACD \U0001FA8A"),
    ("t9-sequenzen", "Hautton \U0001F44D\U0001F3FD Tastenkappe 1\ufe0f\u20e3 Sark \U0001F1E8\U0001F1F6"),
    ("t10-gross-tags", f"Tags gross A{tags('HIDDEN TEXT')}B Sprache \U000E0001{tags('de')}C"),
    ("t11-ohne-schrift", "Ohne Schrift \ue000 \U0001FB00 \U00013000 \u0f00"),
    ("t7-post", f"Jemand auf X: „Tor! {SCHWARZ}{tags('gbeng')}{ENDE}⚽\U0001F525“ / X — Mozilla Firefox"),
]

if __name__ == "__main__":
    import sys
    # --liste: Titel als Escape (für den Nachweis, ohne unsichtbare Zeichen)
    for name, titel in TITEL:
        print(f"{name}\t{ascii(titel) if '--liste' in sys.argv else titel}")
