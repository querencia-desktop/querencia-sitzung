//! Uhrzeit und Datum für den Sperrbildschirm. Wochentage, Monate und die
//! Form des Datums kommen aus dem Katalog (Sitzung 0.1.10), nicht aus der
//! Locale: die Sitzung läuft mit LANG=en_US.UTF-8 und LANGUAGE=<Sprache>,
//! LC_TIME wäre also englisch.

use gtk::glib;

use crate::i18n::{gettext, gettext_f};

/// Name des Wochentags, 1 = Montag … 7 = Sonntag (wie `glib::DateTime`).
fn wochentag_name(wochentag: i32) -> String {
    match wochentag.clamp(1, 7) {
        1 => gettext("Monday"),
        2 => gettext("Tuesday"),
        3 => gettext("Wednesday"),
        4 => gettext("Thursday"),
        5 => gettext("Friday"),
        6 => gettext("Saturday"),
        _ => gettext("Sunday"),
    }
}

/// Name des Monats, 1 … 12.
fn monat_name(monat: i32) -> String {
    match monat.clamp(1, 12) {
        1 => gettext("January"),
        2 => gettext("February"),
        3 => gettext("March"),
        4 => gettext("April"),
        // TRANSLATORS: der Monat (Datum auf dem Sperrbildschirm)
        5 => gettext("May"),
        6 => gettext("June"),
        7 => gettext("July"),
        8 => gettext("August"),
        9 => gettext("September"),
        10 => gettext("October"),
        11 => gettext("November"),
        _ => gettext("December"),
    }
}

/// „Saturday, October 18, 2026“, deutsch „Samstag, 18. Oktober 2026“;
/// `wochentag` 1 = Montag … 7 = Sonntag, `monat` 1 … 12 (wie `glib::DateTime`).
pub fn datum(wochentag: i32, tag: i32, monat: i32, jahr: i32) -> String {
    let (wt, mo) = (wochentag_name(wochentag), monat_name(monat));
    let (tag, jahr) = (tag.to_string(), jahr.to_string());
    gettext_f(
        // TRANSLATORS: Datum auf dem Sperrbildschirm; Reihenfolge und
        // Satzzeichen nach der Sprache (de „{weekday}, {day}. {month} {year}“)
        "{weekday}, {month} {day}, {year}",
        &[
            ("weekday", &wt),
            ("day", &tag),
            ("month", &mo),
            ("year", &jahr),
        ],
    )
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

    // Ohne gebundenen Katalog: englische msgid. Deutsch prüft
    // `deutsches_datum_aus_dem_katalog` über po/de.po.
    #[test]
    fn datum_englisch_ohne_katalog() {
        assert_eq!(datum(6, 18, 10, 2026), "Saturday, October 18, 2026");
        assert_eq!(datum(1, 1, 1, 2027), "Monday, January 1, 2027");
        assert_eq!(datum(7, 31, 12, 2026), "Sunday, December 31, 2026");
        assert_eq!(datum(3, 3, 3, 2027), "Wednesday, March 3, 2027");
    }

    #[test]
    fn datum_ausser_bereich_stuerzt_nicht_ab() {
        assert_eq!(datum(0, 5, 13, 2026), "Monday, December 5, 2026");
        assert_eq!(datum(9, 5, 0, 2026), "Sunday, January 5, 2026");
    }

    /// Die deutsche Form steht im Katalog wie bisher im Code.
    #[test]
    fn deutsches_datum_aus_dem_katalog() {
        let de = include_str!("../po/de.po");
        for (en, de_text) in [
            (
                "{weekday}, {month} {day}, {year}",
                "{weekday}, {day}. {month} {year}",
            ),
            ("Saturday", "Samstag"),
            ("October", "Oktober"),
            ("March", "März"),
            ("May", "Mai"),
        ] {
            let eintrag = format!("msgid \"{en}\"\nmsgstr \"{de_text}\"");
            assert!(de.contains(&eintrag), "{eintrag}");
        }
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
