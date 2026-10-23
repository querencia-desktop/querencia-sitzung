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
use crate::i18n::{gettext, ngettext_f};

pub const APP_ID: &str = "org.querencia.Sitzung";

/// Schaltflächenbeschriftung (mit Mnemonik, übersetzt) und Symbolnamen je
/// Aktion. Wortschatz gnome-session 2.30; die Zugriffstasten sind im Dialog
/// „Sitzung beenden“ zusammen mit „_Abbrechen“ eindeutig.
pub fn knopf_text(aktion: Aktion) -> (String, &'static [&'static str]) {
    // TRANSLATORS: Schaltflächen der Sitzungsdialoge; im Dialog „End Session“
    // stehen alle vier zusammen mit „_Cancel“, Zugriffstasten verschieden halten.
    match aktion {
        Aktion::Abmelden => (gettext("Log _Out"), &["system-log-out", "application-exit"]),
        Aktion::Sperren => (
            gettext("_Lock Screen"),
            &["system-lock-screen", "changes-prevent", "dialog-password"],
        ),
        Aktion::Neustart => (gettext("_Restart"), &["system-reboot", "view-refresh"]),
        Aktion::Herunterfahren => (
            gettext("_Shut Down"),
            &["system-shutdown", "application-exit"],
        ),
    }
}

/// Beschriftung von „Abbrechen“ (mit Mnemonik, übersetzt).
pub fn abbrechen_text() -> String {
    gettext("_Cancel")
}

/// Fenstertitel einer Aktion (übersetzt), auch für Fehlermeldungen.
pub fn titel(aktion: Aktion) -> String {
    match aktion {
        Aktion::Abmelden => gettext("Log Out"),
        Aktion::Sperren => gettext("Lock Screen"),
        Aktion::Neustart => gettext("Restart"),
        Aktion::Herunterfahren => gettext("Shut Down"),
    }
}

/// Titel, Frage und Erläuterung der Rückfrage zu einer Aktion.
pub fn texte(aktion: Aktion) -> (String, String, String) {
    let alle_beendet =
        || gettext("All applications will be closed. Anything not saved there will be lost.");
    match aktion {
        Aktion::Abmelden => (
            titel(aktion),
            gettext("Log out of this system now?"),
            gettext(
                "All applications of this session will be closed. \
                 Anything not saved there will be lost.",
            ),
        ),
        Aktion::Sperren => (
            titel(aktion),
            gettext("Lock the screen now?"),
            gettext(
                "The password is needed to unlock the screen. \
                 The applications keep running.",
            ),
        ),
        Aktion::Neustart => (
            titel(aktion),
            gettext("Restart this system now?"),
            alle_beendet(),
        ),
        // Der Dialog bietet auch „Neu starten“ an (0.1.7, wie GNOME 2).
        Aktion::Herunterfahren => (
            titel(aktion),
            gettext("Shut down or restart this system now?"),
            alle_beendet(),
        ),
    }
}

/// Countdown-Zeile wie in GNOME 2 („You will be automatically logged out in
/// 60 seconds.“).
pub fn countdown_text(aktion: Aktion, sekunden: u32) -> String {
    let n = sekunden as usize;
    // TRANSLATORS: Countdown-Zeile der Rückfragen (gnome-session 2.30).
    match aktion {
        Aktion::Abmelden => ngettext_f(
            "You will be automatically logged out in {n} second.",
            "You will be automatically logged out in {n} seconds.",
            n,
            &[],
        ),
        Aktion::Sperren => ngettext_f(
            "The screen will be locked automatically in {n} second.",
            "The screen will be locked automatically in {n} seconds.",
            n,
            &[],
        ),
        Aktion::Neustart => ngettext_f(
            "This system will be automatically restarted in {n} second.",
            "This system will be automatically restarted in {n} seconds.",
            n,
            &[],
        ),
        Aktion::Herunterfahren => ngettext_f(
            "This system will be automatically shut down in {n} second.",
            "This system will be automatically shut down in {n} seconds.",
            n,
            &[],
        ),
    }
}

/// Höchstens so viele Sekunden am Stück ohne Fokus, dann bricht ein Dialog
/// mit Countdown ab (QD-062).
pub const OHNE_FOKUS_MAX: u32 = 60;

/// Was nach einer Sekunde des Countdowns geschieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schritt {
    /// Mit Fokus heruntergezählt; noch `rest` Sekunden.
    Weiter { rest: u32 },
    /// Ohne Fokus angehalten, seit `sekunden` Sekunden am Stück.
    Angehalten { sekunden: u32 },
    /// Abgelaufen: die Aktion ausführen.
    Ausloesen,
    /// [`OHNE_FOKUS_MAX`] Sekunden ohne Fokus: abbrechen.
    Abbrechen,
}

/// Countdown, der nur zählt, solange der Dialog den Fokus hat (QD-062).
/// Am 20.10.2026 lief ein stehengelassener Abmelden-Dialog hinter einem
/// anderen Fenster ab und meldete ab. Jetzt hält der Countdown an, sobald
/// ein anderes Fenster aktiv ist, zählt bei Rückkehr weiter und bricht nach
/// [`OHNE_FOKUS_MAX`] Sekunden ohne Fokus ab (wie „Abbrechen“).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zaehler {
    rest: u32,
    ohne_fokus: u32,
}

impl Zaehler {
    pub fn neu(sekunden: u32) -> Self {
        Self {
            rest: sekunden,
            ohne_fokus: 0,
        }
    }

    /// Eine Sekunde ist vergangen; `fokus`: der Dialog war aktiv.
    pub fn sekunde(&mut self, fokus: bool) -> Schritt {
        if !fokus {
            self.ohne_fokus += 1;
            return if self.ohne_fokus >= OHNE_FOKUS_MAX {
                Schritt::Abbrechen
            } else {
                Schritt::Angehalten {
                    sekunden: self.ohne_fokus,
                }
            };
        }
        self.ohne_fokus = 0;
        self.rest = self.rest.saturating_sub(1);
        if self.rest == 0 {
            Schritt::Ausloesen
        } else {
            Schritt::Weiter { rest: self.rest }
        }
    }

    pub fn rest(&self) -> u32 {
        self.rest
    }
}

/// Zeile unter dem Countdown, solange er angehalten ist.
pub fn angehalten_text() -> String {
    // TRANSLATORS: steht unter der Countdown-Zeile, solange ein anderes
    // Fenster aktiv ist.
    gettext("Paused while this window is not active.")
}

/// Aufbau eines Dialogs: Titel, Texte, angebotene Aktionen, Countdown-Ziel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aufbau {
    pub titel: String,
    pub frage: String,
    pub detail: String,
    pub symbol: &'static [&'static str],
    /// Schaltflächen von links nach rechts; `None` = Abbrechen.
    pub knoepfe: Vec<Option<Aktion>>,
    /// Vorgabeknopf (GTK-Standardwidget, wie die Vorgabeantwort in
    /// gnome-session 2.30); den Fokus behält „Abbrechen“.
    pub vorgabe: Aktion,
    /// Aktion und Sekunden des Countdowns.
    pub countdown: Option<(Aktion, u32)>,
}

/// Schaltflächen einer Rückfrage von links nach rechts: „Abbrechen“, dann
/// die Aktion. Der Dialog „Herunterfahren“ bietet wie in GNOME 2 auch
/// „Neu starten“ an (0.1.7); Herunterfahren bleibt rechts.
pub fn rueckfrage_knoepfe(aktion: Aktion) -> Vec<Option<Aktion>> {
    match aktion {
        Aktion::Herunterfahren => vec![None, Some(Aktion::Neustart), Some(aktion)],
        _ => vec![None, Some(aktion)],
    }
}

/// [`Aufbau`] für einen Befehl (nur Dialoge).
pub fn aufbau(befehl: &Befehl) -> Option<Aufbau> {
    match befehl {
        Befehl::Dialog { countdown } => Some(Aufbau {
            titel: gettext("End Session"),
            frage: gettext("End this session now?"),
            detail: gettext(
                "Log Out closes all applications of this session; \
                 Restart and Shut Down stop the computer. \
                 Anything not saved will be lost.",
            ),
            symbol: &["system-log-out", "application-exit"],
            knoepfe: vec![
                Some(Aktion::Sperren),
                Some(Aktion::Neustart),
                Some(Aktion::Herunterfahren),
                None,
                Some(Aktion::Abmelden),
            ],
            vorgabe: Aktion::Abmelden,
            countdown: countdown.map(|s| (Aktion::Abmelden, s)),
        }),
        Befehl::Rueckfrage { aktion, countdown } => {
            let (titel, frage, detail) = texte(*aktion);
            Some(Aufbau {
                titel,
                frage,
                detail,
                symbol: knopf_text(*aktion).1,
                knoepfe: rueckfrage_knoepfe(*aktion),
                vorgabe: *aktion,
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

/// Abstände der Schaltflächen; Farben kommen nur aus dem GTK-Thema. Dazu
/// die Abhilfe für den Dunkelmodus beim Start (QD-109).
fn css_laden() {
    crate::farbschema::startfehler_beheben("querencia-sitzung");
    let css = gtk::CssProvider::new();
    css.load_from_string("window.qs-dialog button { padding: 4px 10px; min-width: 72px; }");
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
        .title(aufbau.titel.as_str())
        .resizable(false)
        .build();
    window.add_css_class("qs-dialog");

    let symbol = gtk::Image::from_gicon(&gtk::gio::ThemedIcon::from_names(aufbau.symbol));
    symbol.set_pixel_size(48);
    symbol.set_valign(gtk::Align::Start);
    let frage = gtk::Label::builder()
        .label(format!(
            "<b><big>{}</big></b>",
            glib::markup_escape_text(&aufbau.frage)
        ))
        .use_markup(true)
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(48)
        .build();
    let detail = gtk::Label::builder()
        .label(aufbau.detail.as_str())
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
    let mut abbrechen = None;
    let mut vorgabe = None;
    for eintrag in &aufbau.knoepfe {
        let button = match eintrag {
            Some(aktion) => {
                let (beschriftung, symbole) = knopf_text(*aktion);
                let b = knopf(&beschriftung, symbole);
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
                if aktion == aufbau.vorgabe {
                    vorgabe = Some(b.clone());
                }
                b
            }
            None => {
                let b = knopf(&abbrechen_text(), &["process-stop", "dialog-cancel"]);
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
    window.set_default_widget(vorgabe.as_ref());
    if let Some(abbrechen) = &abbrechen {
        GtkWindowExt::set_focus(&window, Some(abbrechen));
    }

    if let Some((aktion, sekunden)) = aufbau.countdown {
        countdown.set_label(&countdown_text(aktion, sekunden));
        countdown.set_visible(true);
        let zaehler = Rc::new(Cell::new(Zaehler::neu(sekunden)));
        // Fokuswechsel sofort anzeigen, nicht erst beim nächsten Takt.
        window.connect_is_active_notify(glib::clone!(
            #[weak]
            countdown,
            #[strong]
            zaehler,
            move |w| {
                let rest = zaehler.get().rest();
                if w.is_active() {
                    countdown.set_label(&countdown_text(aktion, rest));
                } else {
                    countdown.set_label(&format!(
                        "{}\n{}",
                        countdown_text(aktion, rest),
                        angehalten_text()
                    ));
                }
            }
        ));
        let war_aktiv = Rc::new(Cell::new(true));
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
                    let mut z = zaehler.get();
                    let aktiv = window.is_active();
                    let schritt = z.sekunde(aktiv);
                    zaehler.set(z);
                    if aktiv != war_aktiv.replace(aktiv) {
                        crate::aktion::protokoll(if aktiv {
                            "Countdown läuft weiter (Fenster aktiv)"
                        } else {
                            "Countdown angehalten (Fenster nicht aktiv)"
                        });
                    }
                    match schritt {
                        Schritt::Weiter { rest } => {
                            countdown.set_label(&countdown_text(aktion, rest));
                            glib::ControlFlow::Continue
                        }
                        Schritt::Angehalten { .. } => glib::ControlFlow::Continue,
                        Schritt::Ausloesen => {
                            wahl.set(Some(aktion));
                            window.close();
                            glib::ControlFlow::Break
                        }
                        Schritt::Abbrechen => {
                            crate::aktion::protokoll(&format!(
                                "Countdown: {OHNE_FOKUS_MAX} s ohne Fokus, Dialog abgebrochen"
                            ));
                            window.close();
                            glib::ControlFlow::Break
                        }
                    }
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
        let ok = knopf(&gettext("_Close"), &["window-close"]);
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
        assert_eq!(a.vorgabe, Aktion::Abmelden);
    }

    #[test]
    fn herunterfahren_bietet_neu_starten_an() {
        for countdown in [None, Some(60)] {
            let a = aufbau(&Befehl::Rueckfrage {
                aktion: Aktion::Herunterfahren,
                countdown,
            })
            .unwrap();
            assert_eq!(
                a.knoepfe,
                vec![None, Some(Aktion::Neustart), Some(Aktion::Herunterfahren)]
            );
            assert_eq!(a.vorgabe, Aktion::Herunterfahren);
            assert_eq!(a.countdown, countdown.map(|s| (Aktion::Herunterfahren, s)));
            // Ohne Katalog englisch (msgid), deutsch siehe po/de.po.
            assert_eq!(a.frage, "Shut down or restart this system now?");
            assert_eq!(a.titel, "Shut Down");
        }
    }

    #[test]
    fn andere_rueckfragen_nur_abbrechen_und_aktion() {
        for aktion in [Aktion::Abmelden, Aktion::Sperren, Aktion::Neustart] {
            assert_eq!(rueckfrage_knoepfe(aktion), vec![None, Some(aktion)]);
        }
    }

    /// Buchstabe hinter dem `_` einer Beschriftung, klein.
    fn zugriffstaste(text: &str) -> char {
        let i = text.find('_').expect("Zugriffstaste") + 1;
        text[i..]
            .chars()
            .next()
            .unwrap()
            .to_lowercase()
            .next()
            .unwrap()
    }

    /// `msgstr` zu `msgid` aus einer .po-Datei (nur einzeilige Einträge).
    fn uebersetzung(po: &str, msgid: &str) -> String {
        let kopf = format!("msgid \"{msgid}\"\nmsgstr \"");
        let rest = &po[po.find(&kopf).expect("msgid in po") + kopf.len()..];
        rest[..rest.find('"').unwrap()].to_string()
    }

    #[test]
    fn zugriffstasten_im_dialog_herunterfahren_verschieden() {
        let texte: Vec<String> = rueckfrage_knoepfe(Aktion::Herunterfahren)
            .into_iter()
            .map(|k| match k {
                Some(a) => knopf_text(a).0,
                None => abbrechen_text(),
            })
            .collect();
        let de = include_str!("../po/de.po");
        for sprache in ["en", "de"] {
            let mut tasten: Vec<char> = texte
                .iter()
                .map(|t| match sprache {
                    "de" => zugriffstaste(&uebersetzung(de, t)),
                    _ => zugriffstaste(t),
                })
                .collect();
            tasten.sort_unstable();
            tasten.dedup();
            assert_eq!(tasten.len(), 3, "{sprache}: {tasten:?}");
        }
        assert_eq!(uebersetzung(de, "_Restart"), "Neu s_tarten");
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
            assert_eq!(a.knoepfe.first(), Some(&None), "Abbrechen links");
            assert_eq!(a.knoepfe.last(), Some(&Some(aktion)), "Aktion rechts");
            assert_eq!(a.vorgabe, aktion);
            assert_eq!(a.countdown, Some((aktion, 60)));
            assert_eq!(a.titel, titel(aktion));
            assert!(a.frage.ends_with('?'));
        }
        assert_eq!(aufbau(&Befehl::SperrenSofort), None);
        assert_eq!(aufbau(&Befehl::Version), None);
    }

    #[test]
    fn countdown_texte() {
        // Ohne Katalog englisch (msgid), deutsch siehe po/de.po.
        assert_eq!(
            countdown_text(Aktion::Abmelden, 60),
            "You will be automatically logged out in 60 seconds."
        );
        assert_eq!(
            countdown_text(Aktion::Neustart, 1),
            "This system will be automatically restarted in 1 second."
        );
        assert!(countdown_text(Aktion::Herunterfahren, 5).contains("shut down"));
        assert!(countdown_text(Aktion::Sperren, 5).contains("locked"));
    }

    #[test]
    fn countdown_zaehlt_nur_mit_fokus() {
        let mut z = Zaehler::neu(3);
        assert_eq!(z.sekunde(true), Schritt::Weiter { rest: 2 });
        assert_eq!(z.sekunde(false), Schritt::Angehalten { sekunden: 1 });
        assert_eq!(z.sekunde(false), Schritt::Angehalten { sekunden: 2 });
        assert_eq!(z.rest(), 2);
        assert_eq!(z.sekunde(true), Schritt::Weiter { rest: 1 });
        assert_eq!(z.sekunde(true), Schritt::Ausloesen);
    }

    #[test]
    fn countdown_bricht_nach_sechzig_sekunden_ohne_fokus_ab() {
        let mut z = Zaehler::neu(60);
        for s in 1..OHNE_FOKUS_MAX {
            assert_eq!(z.sekunde(false), Schritt::Angehalten { sekunden: s });
        }
        assert_eq!(z.sekunde(false), Schritt::Abbrechen);
        assert_eq!(z.rest(), 60, "ohne Fokus nichts heruntergezählt");
    }

    #[test]
    fn fokus_setzt_die_frist_ohne_fokus_zurueck() {
        let mut z = Zaehler::neu(600);
        for _ in 0..OHNE_FOKUS_MAX - 1 {
            z.sekunde(false);
        }
        assert_eq!(z.sekunde(true), Schritt::Weiter { rest: 599 });
        for s in 1..OHNE_FOKUS_MAX {
            assert_eq!(z.sekunde(false), Schritt::Angehalten { sekunden: s });
        }
        assert_eq!(z.sekunde(false), Schritt::Abbrechen);
    }

    #[test]
    fn angehalten_hat_einen_text() {
        assert_eq!(angehalten_text(), "Paused while this window is not active.");
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
