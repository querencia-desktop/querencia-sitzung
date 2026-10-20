//! Benutzereinstellung der Sitzungsdialoge (QD-062):
//! `$XDG_CONFIG_HOME/querencia/sitzung.ini` (sonst `~/.config/…`).
//!
//! ```text
//! [Dialog]
//! Countdown=an      # an | aus
//! ```
//!
//! `Countdown=an` gibt Abmelden, Neu starten und Herunterfahren einen
//! Countdown von 60 s (er zählt nur mit Fokus, siehe `dialog.rs`). Ohne
//! Datei oder Eintrag gilt [`COUNTDOWN_VORGABE`]: kein Countdown, die
//! Rückfrage wartet, bis jemand wählt. `--countdown` auf der Befehlszeile
//! geht der Datei vor. Das Programm schreibt die Datei nie.

use std::path::PathBuf;

/// Countdown ohne Eintrag in der Datei: aus (Entscheidung von pottrauschen,
/// 20.10.2026: „lass countdown weg“).
pub const COUNTDOWN_VORGABE: bool = false;

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

/// `Countdown` im Abschnitt `[Dialog]` von `text`: `Some(true|false)` bei
/// gültigem Wert, sonst `None` (dann gilt die Vorgabe). Spätere Einträge
/// gehen früheren vor; `#` und `;` leiten Kommentare ein.
pub fn countdown_aus_text(text: &str) -> Option<bool> {
    let mut abschnitt = String::new();
    let mut wert = None;
    for zeile in text.lines() {
        let zeile = zeile.split(['#', ';']).next().unwrap_or("").trim();
        if let Some(name) = zeile.strip_prefix('[').and_then(|z| z.strip_suffix(']')) {
            abschnitt = name.trim().to_string();
            continue;
        }
        let Some((schluessel, v)) = zeile.split_once('=') else {
            continue;
        };
        if abschnitt != "Dialog" || schluessel.trim() != "Countdown" {
            continue;
        }
        match v.trim().to_ascii_lowercase().as_str() {
            "an" | "ja" | "true" | "1" | "on" => wert = Some(true),
            "aus" | "nein" | "false" | "0" | "off" => wert = Some(false),
            _ => {}
        }
    }
    wert
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
            countdown_aus_text("[Dialog]\nCountdown=AUS # pottrauschen\n"),
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

    // Vorgabe: kein Countdown (beim Übersetzen geprüft).
    const _: () = assert!(!COUNTDOWN_VORGABE);

    #[test]
    fn datei_liegt_unter_querencia() {
        assert!(pfad().ends_with("querencia/sitzung.ini"));
    }
}
