//! Benutzereinstellung der Sitzungsdialoge (QD-062):
//! `$XDG_CONFIG_HOME/querencia/sitzung.ini` (sonst `~/.config/…`).
//!
//! ```text
//! [Dialog]
//! Countdown=an      # an | aus
//! [Sperre]
//! BildschirmAus=2min   # Sekunden (120, 120s) oder Minuten (2min); 0 = aus
//! ```
//!
//! `Countdown=an` gibt Abmelden, Neu starten und Herunterfahren einen
//! Countdown von 60 s (er zählt nur mit Fokus, siehe `dialog.rs`). Ohne
//! Datei oder Eintrag gilt [`COUNTDOWN_VORGABE`]: kein Countdown, die
//! Rückfrage wartet, bis jemand wählt. `--countdown` auf der Befehlszeile
//! geht der Datei vor.
//!
//! `BildschirmAus` (0.1.6): So lange nach dem Sperren ohne Eingabe schaltet
//! der Leerlaufdienst die Ausgänge ab, ohne Eintrag nach
//! [`BILDSCHIRM_AUS_VORGABE_S`]. Das Programm schreibt die Datei nie.

use std::path::PathBuf;

/// Countdown ohne Eintrag in der Datei: aus (Entscheidung des Besitzers,
/// 20.10.2026: „lass countdown weg“).
pub const COUNTDOWN_VORGABE: bool = false;

/// Bildschirm aus nach der Sperre ohne Eintrag: 2 Minuten (Auftrag 0.1.6).
pub const BILDSCHIRM_AUS_VORGABE_S: u32 = 120;

/// Längste Wartezeit, die angenommen wird (24 h); längere Werte gelten als
/// ungültig.
const BILDSCHIRM_AUS_HOECHSTENS_S: u32 = 24 * 60 * 60;

/// Ort der Datei.
pub fn pfad() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    config.join("querencia").join("sitzung.ini")
}

/// Alle Werte von `schluessel` im Abschnitt `[abschnitt]` von `text`, in
/// ihrer Reihenfolge; `#` und `;` leiten Kommentare ein.
fn werte<'a>(text: &'a str, abschnitt: &str, schluessel: &str) -> Vec<&'a str> {
    let mut aktuell = "";
    let mut aus = Vec::new();
    for zeile in text.lines() {
        let zeile = zeile.split(['#', ';']).next().unwrap_or("").trim();
        if let Some(name) = zeile.strip_prefix('[').and_then(|z| z.strip_suffix(']')) {
            aktuell = name.trim();
            continue;
        }
        let Some((k, v)) = zeile.split_once('=') else {
            continue;
        };
        if aktuell == abschnitt && k.trim() == schluessel {
            aus.push(v.trim());
        }
    }
    aus
}

/// `Countdown` im Abschnitt `[Dialog]` von `text`: `Some(true|false)` bei
/// gültigem Wert, sonst `None` (dann gilt die Vorgabe). Spätere Einträge
/// gehen früheren vor; `#` und `;` leiten Kommentare ein.
pub fn countdown_aus_text(text: &str) -> Option<bool> {
    let mut wert = None;
    for v in werte(text, "Dialog", "Countdown") {
        match v.to_ascii_lowercase().as_str() {
            "an" | "ja" | "true" | "1" | "on" => wert = Some(true),
            "aus" | "nein" | "false" | "0" | "off" => wert = Some(false),
            _ => {}
        }
    }
    wert
}

/// Eine Wartezeit: `120`, `120s` (Sekunden) oder `2min`, `2m` (Minuten).
fn dauer_s(v: &str) -> Option<u32> {
    let v = v.trim().to_ascii_lowercase();
    let (zahl, faktor) = if let Some(z) = v.strip_suffix("min").or_else(|| v.strip_suffix('m')) {
        (z, 60)
    } else if let Some(z) = v.strip_suffix('s') {
        (z, 1)
    } else {
        (v.as_str(), 1)
    };
    let s = zahl.trim().parse::<u32>().ok()?.checked_mul(faktor)?;
    (s <= BILDSCHIRM_AUS_HOECHSTENS_S).then_some(s)
}

/// `BildschirmAus` im Abschnitt `[Sperre]` von `text` in Sekunden (0 = aus),
/// `None` ohne gültigen Eintrag. Spätere Einträge gehen früheren vor.
pub fn bildschirm_aus_aus_text(text: &str) -> Option<u32> {
    werte(text, "Sperre", "BildschirmAus")
        .into_iter()
        .filter_map(dauer_s)
        .next_back()
}

/// Sekunden bis „Bildschirm aus“ nach der Sperre (0 = nie), mit Herkunft.
pub fn bildschirm_aus() -> (u32, String) {
    let pfad = pfad();
    match std::fs::read_to_string(&pfad) {
        Ok(text) => match bildschirm_aus_aus_text(&text) {
            Some(s) => (s, pfad.display().to_string()),
            None => (
                BILDSCHIRM_AUS_VORGABE_S,
                format!("Vorgabe, kein gültiger Eintrag in {}", pfad.display()),
            ),
        },
        Err(_) => (BILDSCHIRM_AUS_VORGABE_S, "Vorgabe".to_string()),
    }
}

/// Ob die Dialoge herunterzählen sollen, mit Herkunft für das Protokoll.
pub fn countdown() -> (bool, String) {
    let pfad = pfad();
    match std::fs::read_to_string(&pfad) {
        Ok(text) => match countdown_aus_text(&text) {
            Some(an) => (an, pfad.display().to_string()),
            None => (
                COUNTDOWN_VORGABE,
                format!("Vorgabe, kein gültiger Eintrag in {}", pfad.display()),
            ),
        },
        Err(_) => (COUNTDOWN_VORGABE, "Vorgabe".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn werte_und_abschnitte() {
        assert_eq!(countdown_aus_text(""), None);
        assert_eq!(countdown_aus_text("[Dialog]\nCountdown=aus\n"), Some(false));
        assert_eq!(countdown_aus_text("[Dialog]\nCountdown = an\n"), Some(true));
        assert_eq!(
            countdown_aus_text("[Dialog]\nCountdown=AUS # Notiz\n"),
            Some(false)
        );
        assert_eq!(countdown_aus_text("[Anderes]\nCountdown=aus\n"), None);
        assert_eq!(countdown_aus_text("Countdown=aus\n"), None);
        assert_eq!(countdown_aus_text("[Dialog]\nCountdown=vielleicht\n"), None);
        assert_eq!(
            countdown_aus_text("[Dialog]\nCountdown=aus\n[Dialog]\nCountdown=an\n"),
            Some(true)
        );
        assert_eq!(countdown_aus_text("[Dialog]\n;Countdown=aus\n"), None);
    }

    #[test]
    fn bildschirm_aus_werte() {
        assert_eq!(bildschirm_aus_aus_text(""), None);
        assert_eq!(
            bildschirm_aus_aus_text("[Sperre]\nBildschirmAus=0\n"),
            Some(0)
        );
        assert_eq!(
            bildschirm_aus_aus_text("[Sperre]\nBildschirmAus=120\n"),
            Some(120)
        );
        assert_eq!(
            bildschirm_aus_aus_text("[Sperre]\nBildschirmAus = 45s\n"),
            Some(45)
        );
        assert_eq!(
            bildschirm_aus_aus_text("[Sperre]\nBildschirmAus=2min # Notiz\n"),
            Some(120)
        );
        assert_eq!(
            bildschirm_aus_aus_text("[Sperre]\nBildschirmAus=5m\n"),
            Some(300)
        );
        assert_eq!(
            bildschirm_aus_aus_text("[Sperre]\nBildschirmAus=bald\n"),
            None
        );
        assert_eq!(
            bildschirm_aus_aus_text("[Sperre]\nBildschirmAus=-1\n"),
            None
        );
        assert_eq!(
            bildschirm_aus_aus_text("[Sperre]\nBildschirmAus=1441min\n"),
            None
        );
        assert_eq!(
            bildschirm_aus_aus_text("[Dialog]\nBildschirmAus=10\n"),
            None
        );
        assert_eq!(
            bildschirm_aus_aus_text(
                "[Sperre]\nBildschirmAus=10\nBildschirmAus=kaputt\nBildschirmAus=20\n"
            ),
            Some(20)
        );
        // Countdown und BildschirmAus in einer Datei
        let beides = "[Dialog]\nCountdown=an\n[Sperre]\nBildschirmAus=30\n";
        assert_eq!(countdown_aus_text(beides), Some(true));
        assert_eq!(bildschirm_aus_aus_text(beides), Some(30));
    }

    const _: () = assert!(BILDSCHIRM_AUS_VORGABE_S == 120);

    // Vorgabe: kein Countdown (beim Übersetzen geprüft).
    const _: () = assert!(!COUNTDOWN_VORGABE);

    #[test]
    fn datei_liegt_unter_querencia() {
        assert!(pfad().ends_with("querencia/sitzung.ini"));
    }
}
