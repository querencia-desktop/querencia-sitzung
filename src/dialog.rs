//! Rückfragen im Stil des GNOME-2-Abmeldedialogs (gnome-session 2.30):
//! Symbol links, fette Frage, Erläuterung, Countdown-Zeile, Schaltflächen
//! rechts unten, „Abbrechen“ hat den Fokus. Esc bricht ab, Pfeiltasten
//! wandern zwischen den Schaltflächen, Enter löst die markierte aus.
//! GTK 4 ohne libadwaita; labwc zeichnet Rahmen und Titelleiste.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::cli::{Aktion, Befehl};

pub const APP_ID: &str = "org.querencia.Sitzung";

/// Schaltflächenbeschriftung (mit Mnemonik) und Symbolnamen je Aktion.
pub fn knopf_text(aktion: Aktion) -> (&'static str, &'static [&'static str]) {
    match aktion {
        Aktion::Abmelden => ("Ab_melden", &["system-log-out", "application-exit"]),
        Aktion::Sperren => (
            "Bildschirm _sperren",
            &["system-lock-screen", "changes-prevent", "dialog-password"],
        ),
        Aktion::Neustart => ("Neu s_tarten", &["system-reboot", "view-refresh"]),
        Aktion::Herunterfahren => ("_Herunterfahren", &["system-shutdown", "application-exit"]),
    }
}

/// Titel, Frage und Erläuterung der Rückfrage zu einer Aktion.
pub fn texte(aktion: Aktion) -> (&'static str, &'static str, &'static str) {
    match aktion {
        Aktion::Abmelden => (
            "Abmelden",
            "Möchten Sie sich jetzt abmelden?",
            "Alle Anwendungen dieser Sitzung werden beendet. \
             Was dort nicht gespeichert ist, geht verloren.",
        ),
        Aktion::Sperren => (
            "Bildschirm sperren",
            "Möchten Sie den Bildschirm jetzt sperren?",
            "Zum Entsperren wird Ihr Passwort benötigt. \
             Die Anwendungen laufen weiter.",
        ),
        Aktion::Neustart => (
            "Neu starten",
            "Möchten Sie den Rechner jetzt neu starten?",
            "Alle Anwendungen werden beendet. \
             Was dort nicht gespeichert ist, geht verloren.",
        ),
        Aktion::Herunterfahren => (
            "Herunterfahren",
            "Möchten Sie den Rechner jetzt herunterfahren?",
            "Alle Anwendungen werden beendet. \
             Was dort nicht gespeichert ist, geht verloren.",
        ),
    }
}

/// Countdown-Zeile wie in GNOME 2 („Sie werden in 60 Sekunden automatisch
/// abgemeldet.“).
pub fn countdown_text(aktion: Aktion, sekunden: u32) -> String {
    let zeit = if sekunden == 1 {
        "1 Sekunde".to_string()
    } else {
        format!("{sekunden} Sekunden")
    };
    match aktion {
        Aktion::Abmelden => format!("Sie werden in {zeit} automatisch abgemeldet."),
        Aktion::Sperren => format!("Der Bildschirm wird in {zeit} automatisch gesperrt."),
        Aktion::Neustart => format!("Der Rechner wird in {zeit} automatisch neu gestartet."),
        Aktion::Herunterfahren => {
            format!("Der Rechner wird in {zeit} automatisch heruntergefahren.")
        }
    }
}

/// Aufbau eines Dialogs: Titel, Texte, angebotene Aktionen, Countdown-Ziel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aufbau {
    pub titel: &'static str,
    pub frage: &'static str,
    pub detail: &'static str,
    pub symbol: &'static [&'static str],
    /// Schaltflächen von links nach rechts; `None` = Abbrechen.
    pub knoepfe: Vec<Option<Aktion>>,
    /// Aktion und Sekunden des Countdowns.
    pub countdown: Option<(Aktion, u32)>,
}

/// [`Aufbau`] für einen Befehl (nur Dialoge).
pub fn aufbau(befehl: &Befehl) -> Option<Aufbau> {
    match befehl {
        Befehl::Dialog { countdown } => Some(Aufbau {
            titel: "Sitzung beenden",
            frage: "Diese Sitzung jetzt beenden?",
            detail: "Abmelden beendet alle Anwendungen dieser Sitzung; \
                     Neu starten und Herunterfahren beenden den Rechner. \
                     Was nicht gespeichert ist, geht verloren.",
            symbol: &["system-log-out", "application-exit"],
            knoepfe: vec![
                Some(Aktion::Sperren),
                Some(Aktion::Neustart),
                Some(Aktion::Herunterfahren),
                None,
                Some(Aktion::Abmelden),
            ],
            countdown: countdown.map(|s| (Aktion::Abmelden, s)),
        }),
        Befehl::Rueckfrage { aktion, countdown } => {
            let (titel, frage, detail) = texte(*aktion);
            Some(Aufbau {
                titel,
                frage,
                detail,
                symbol: knopf_text(*aktion).1,
                knoepfe: vec![None, Some(*aktion)],
                countdown: countdown.map(|s| (*aktion, s)),
            })
        }
        _ => None,
    }
}

/// Schaltfläche mit Symbol und Mnemonik-Beschriftung (wie GTK-2-Stock).
fn knopf(beschriftung: &str, symbole: &[&str]) -> gtk::Button {
    let button = gtk::Button::new();
    let inhalt = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let bild = gtk::Image::from_gicon(&gtk::gio::ThemedIcon::from_names(symbole));
    bild.set_pixel_size(16);
    let label = gtk::Label::builder()
        .label(beschriftung)
        .use_underline(true)
        .mnemonic_widget(&button)
        .build();
    inhalt.append(&bild);
    inhalt.append(&label);
    inhalt.set_halign(gtk::Align::Center);
    button.set_child(Some(&inhalt));
    button
}

fn css_laden() {
    let css = gtk::CssProvider::new();
    css.load_from_string("window.qs-dialog button { padding: 4px 12px; min-width: 96px; }");
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Baut das Fenster; `wahl` erhält die gewählte Aktion, `None` bei Abbrechen.
fn fenster(app: &gtk::Application, aufbau: &Aufbau, wahl: Rc<Cell<Option<Aktion>>>) -> gtk::Window {
    let window = gtk::Window::builder()
        .application(app)
        .title(aufbau.titel)
        .resizable(false)
        .build();
    window.add_css_class("qs-dialog");

    let symbol = gtk::Image::from_gicon(&gtk::gio::ThemedIcon::from_names(aufbau.symbol));
    symbol.set_pixel_size(48);
    symbol.set_valign(gtk::Align::Start);
    let frage = gtk::Label::builder()
        .label(format!(
            "<b><big>{}</big></b>",
            glib::markup_escape_text(aufbau.frage)
        ))
        .use_markup(true)
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(48)
        .build();
    let detail = gtk::Label::builder()
        .label(aufbau.detail)
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(48)
        .build();
    let countdown = gtk::Label::builder().xalign(0.0).wrap(true).build();
    countdown.set_visible(false);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 8);
    text.append(&frage);
    text.append(&detail);
    text.append(&countdown);
    let oben = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    oben.append(&symbol);
    oben.append(&text);

    let knoepfe = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    knoepfe.set_halign(gtk::Align::End);
    knoepfe.set_homogeneous(true);
    let mut abbrechen = None;
    for eintrag in &aufbau.knoepfe {
        let button = match eintrag {
            Some(aktion) => {
                let (beschriftung, symbole) = knopf_text(*aktion);
                let b = knopf(beschriftung, symbole);
                let aktion = *aktion;
                b.connect_clicked(glib::clone!(
                    #[weak]
                    window,
                    #[strong]
                    wahl,
                    move |_| {
                        wahl.set(Some(aktion));
                        window.close();
                    }
                ));
                b
            }
            None => {
                let b = knopf("_Abbrechen", &["process-stop", "dialog-cancel"]);
                b.connect_clicked(glib::clone!(
                    #[weak]
                    window,
                    move |_| window.close()
                ));
                abbrechen = Some(b.clone());
                b
            }
        };
        knoepfe.append(&button);
    }

    let inhalt = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    inhalt.append(&oben);
    inhalt.append(&knoepfe);
    window.set_child(Some(&inhalt));

    // Esc = Abbrechen, wie in GNOME 2.
    let tasten = gtk::EventControllerKey::new();
    tasten.connect_key_pressed(glib::clone!(
        #[weak]
        window,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if key == gdk::Key::Escape {
                window.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    ));
    window.add_controller(tasten);
    if let Some(abbrechen) = &abbrechen {
        GtkWindowExt::set_focus(&window, Some(abbrechen));
    }

    if let Some((aktion, sekunden)) = aufbau.countdown {
        countdown.set_label(&countdown_text(aktion, sekunden));
        countdown.set_visible(true);
        let rest = Rc::new(Cell::new(sekunden));
        glib::timeout_add_seconds_local(
            1,
            glib::clone!(
                #[weak]
                window,
                #[weak]
                countdown,
                #[strong]
                wahl,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    let n = rest.get().saturating_sub(1);
                    rest.set(n);
                    if n == 0 {
                        wahl.set(Some(aktion));
                        window.close();
                        return glib::ControlFlow::Break;
                    }
                    countdown.set_label(&countdown_text(aktion, n));
                    glib::ControlFlow::Continue
                }
            ),
        );
    }
    window
}

/// Zeigt die Rückfrage und liefert die gewählte Aktion, `None` bei
/// Abbrechen (Esc, Schaltfläche, Fenster schließen). Läuft ein Dialog
/// dieser Anwendung bereits, wird er nur nach vorn geholt.
pub fn rueckfrage(aufbau: Aufbau) -> Option<Aktion> {
    let wahl = Rc::new(Cell::new(None));
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| css_laden());
    let aufbau = Rc::new(aufbau);
    app.connect_activate(glib::clone!(
        #[strong]
        wahl,
        move |app| {
            if let Some(offen) = app.active_window() {
                offen.present();
                return;
            }
            fenster(app, &aufbau, wahl.clone()).present();
        }
    ));
    app.run_with_args::<String>(&[]);
    wahl.get()
}

/// Fehlermeldung in einem eigenen kleinen Fenster (z. B. logind verweigert).
pub fn fehler_anzeigen(titel: &str, text: &str) {
    let app = gtk::Application::builder()
        .application_id("org.querencia.Sitzung.Meldung")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let titel = titel.to_string();
    let text = text.to_string();
    app.connect_startup(|_| css_laden());
    app.connect_activate(move |app| {
        let window = gtk::Window::builder()
            .application(app)
            .title(&titel)
            .resizable(false)
            .build();
        window.add_css_class("qs-dialog");
        let symbol = gtk::Image::from_icon_name("dialog-error");
        symbol.set_pixel_size(48);
        symbol.set_valign(gtk::Align::Start);
        let label = gtk::Label::builder()
            .label(&text)
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(48)
            .selectable(true)
            .build();
        let oben = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        oben.append(&symbol);
        oben.append(&label);
        let ok = knopf("_Schließen", &["window-close"]);
        ok.connect_clicked(glib::clone!(
            #[weak]
            window,
            move |_| window.close()
        ));
        ok.set_halign(gtk::Align::End);
        let inhalt = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        inhalt.append(&oben);
        inhalt.append(&ok);
        window.set_child(Some(&inhalt));
        let tasten = gtk::EventControllerKey::new();
        tasten.connect_key_pressed(glib::clone!(
            #[weak]
            window,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    window.close();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        ));
        window.add_controller(tasten);
        GtkWindowExt::set_focus(&window, Some(&ok));
        window.present();
    });
    app.run_with_args::<String>(&[]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gesamtdialog_hat_fuenf_knoepfe_abbrechen_vor_abmelden() {
        let a = aufbau(&Befehl::Dialog { countdown: None }).unwrap();
        assert_eq!(a.knoepfe.len(), 5);
        assert_eq!(a.knoepfe[3], None);
        assert_eq!(a.knoepfe[4], Some(Aktion::Abmelden));
        assert_eq!(a.countdown, None);
        let a = aufbau(&Befehl::Dialog {
            countdown: Some(60),
        })
        .unwrap();
        assert_eq!(a.countdown, Some((Aktion::Abmelden, 60)));
    }

    #[test]
    fn rueckfrage_hat_abbrechen_und_aktion() {
        for aktion in [
            Aktion::Abmelden,
            Aktion::Sperren,
            Aktion::Neustart,
            Aktion::Herunterfahren,
        ] {
            let a = aufbau(&Befehl::Rueckfrage {
                aktion,
                countdown: Some(60),
            })
            .unwrap();
            assert_eq!(a.knoepfe, vec![None, Some(aktion)]);
            assert_eq!(a.countdown, Some((aktion, 60)));
            assert_eq!(a.titel, aktion.name());
            assert!(a.frage.ends_with('?'));
        }
        assert_eq!(aufbau(&Befehl::SperrenSofort), None);
        assert_eq!(aufbau(&Befehl::Version), None);
    }

    #[test]
    fn countdown_texte() {
        assert_eq!(
            countdown_text(Aktion::Abmelden, 60),
            "Sie werden in 60 Sekunden automatisch abgemeldet."
        );
        assert_eq!(
            countdown_text(Aktion::Neustart, 1),
            "Der Rechner wird in 1 Sekunde automatisch neu gestartet."
        );
        assert!(countdown_text(Aktion::Herunterfahren, 5).contains("heruntergefahren"));
        assert!(countdown_text(Aktion::Sperren, 5).contains("gesperrt"));
    }

    #[test]
    fn jede_aktion_hat_mnemonik() {
        for aktion in [
            Aktion::Abmelden,
            Aktion::Sperren,
            Aktion::Neustart,
            Aktion::Herunterfahren,
        ] {
            let (text, symbole) = knopf_text(aktion);
            assert!(text.contains('_'), "{text}");
            assert!(!symbole.is_empty());
        }
    }
}
