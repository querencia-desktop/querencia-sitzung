//! Uhrzeit und Datum auf Deutsch für den Sperrbildschirm (ohne Locale-
//! Abhängigkeit: die Sitzung läuft mit LANG=en_US.UTF-8 und LANGUAGE=de).

use gtk::glib;

const WOCHENTAGE: [&str; 7] = [
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];
const MONATE: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

/// „Samstag, 18. Oktober 2026“; `wochentag` 1 = Montag … 7 = Sonntag,
/// `monat` 1 … 12 (wie `glib::DateTime`).
pub fn datum(wochentag: i32, tag: i32, monat: i32, jahr: i32) -> String {
    let wt = WOCHENTAGE
        .get((wochentag - 1).clamp(0, 6) as usize)
        .copied()
        .unwrap_or("");
    let mo = MONATE
        .get((monat - 1).clamp(0, 11) as usize)
        .copied()
        .unwrap_or("");
    format!("{wt}, {tag}. {mo} {jahr}")
}

/// Uhrzeit „HH:MM“ (24 Stunden).
pub fn uhrzeit(stunde: i32, minute: i32) -> String {
    format!("{stunde:02}:{minute:02}")
}

/// Uhrzeit und Datum von jetzt (Ortszeit).
pub fn jetzt() -> (String, String) {
    match glib::DateTime::now_local() {
        Ok(t) => (
            uhrzeit(t.hour(), t.minute()),
            datum(t.day_of_week(), t.day_of_month(), t.month(), t.year()),
        ),
        Err(_) => ("--:--".to_string(), String::new()),
    }
}

/// Millisekunden bis zum nächsten vollen Minutenwechsel (die Uhr zeigt
/// keine Sekunden); höchstens 60 000, mindestens 50.
pub fn bis_zur_naechsten_minute() -> u64 {
    match glib::DateTime::now_local() {
        Ok(t) => {
            let rest = 60.0 - t.seconds();
            ((rest * 1000.0) as u64).clamp(50, 60_000)
        }
        Err(_) => 1000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deutsches_datum() {
        assert_eq!(datum(6, 18, 10, 2026), "Samstag, 18. Oktober 2026");
        assert_eq!(datum(1, 1, 1, 2027), "Montag, 1. Januar 2027");
        assert_eq!(datum(7, 31, 12, 2026), "Sonntag, 31. Dezember 2026");
        assert_eq!(datum(3, 3, 3, 2027), "Mittwoch, 3. März 2027");
    }

    #[test]
    fn datum_ausser_bereich_stuerzt_nicht_ab() {
        assert_eq!(datum(0, 5, 13, 2026), "Montag, 5. Dezember 2026");
        assert_eq!(datum(9, 5, 0, 2026), "Sonntag, 5. Januar 2026");
    }

    #[test]
    fn uhrzeit_zweistellig() {
        assert_eq!(uhrzeit(7, 5), "07:05");
        assert_eq!(uhrzeit(23, 59), "23:59");
        assert_eq!(uhrzeit(0, 0), "00:00");
    }

    #[test]
    fn wartezeit_in_grenzen() {
        let ms = bis_zur_naechsten_minute();
        assert!((50..=60_000).contains(&ms));
    }
}
