//! Dunkelmodus der Sitzungsdialoge und des polkit-Agenten (QD-109).
//!
//! Die Dialoge haben keine eigenen Farben: Grund, Schrift und Schaltflächen
//! kommen aus dem GTK-Thema (Querencia-Klassisch, ab Thema 0.2 mit
//! Dunkelfassung), das eigene CSS legt nur Abstände fest. Trotzdem blieben
//! sie unter „Dunkle Darstellung bevorzugen“ hell. Ursache ist ein Fehler
//! von GTK 4.22 (Thema 0.2, isoliert gemessen, auch hier
//! `nachweis/014/dunkel-vorher-013`): Ein Programm, das bei schon dunklem
//! Farbschema startet, zeigt das helle Thema, bis die Einstellung
//! `gtk-interface-color-scheme` erneut gemeldet wird. Ein Wechsel im
//! laufenden Programm wirkt. Abhilfe wie im Panel (`farbschema.rs` in
//! querencia-runtime): die Einstellung einmal nach dem Start neu melden.

use gtk::prelude::*;

const SCHEMA: &str = "gtk-interface-color-scheme";

/// Meldet `gtk-interface-color-scheme` einmal neu, damit GTK die
/// Dunkelfassung des Themas lädt; `programm` fürs Protokoll. Ohne die
/// Einstellung (GTK vor 4.20) geschieht nichts.
pub fn startfehler_beheben(programm: &str) {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    if settings.find_property(SCHEMA).is_none() {
        return;
    }
    let wert = settings
        .property_value(SCHEMA)
        .transform::<i32>()
        .ok()
        .and_then(|v| v.get::<i32>().ok())
        .unwrap_or(0);
    eprintln!(
        "{programm}: Farbschema {} (Einstellung {wert}, Thema {}), neu gemeldet",
        match wert {
            2 => "dunkel",
            3 => "hell",
            _ => "Vorgabe",
        },
        settings.gtk_theme_name().unwrap_or_default()
    );
    settings.notify(SCHEMA);
}
