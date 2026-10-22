//! `org.freedesktop.ScreenSaver` auf dem Sitzungsbus (QD-104), angeboten
//! vom Leerlaufdienst (`leerlauf.rs`).
//!
//! Browser und Player halten darüber die Leerlaufsperre an, solange ein
//! Video läuft: direkt (Firefox als Deb unter `/ScreenSaver`, Chromium unter
//! `/org/freedesktop/ScreenSaver`) oder über `xdg-desktop-portal-gtk`. Das
//! Portal fragt `org.gnome.SessionManager` nur, wenn dieser Name einen
//! Besitzer hat, sonst `org.freedesktop.ScreenSaver` („Using
//! org.freedesktop.ScreenSaver for inhibit“ und „… for screensaver state“).
//! Ohne den Dienst meldete es am 21.10.2026 etwa alle 10 s „Backend call
//! failed“. `org.gnome.SessionManager` bietet die Sitzung bewusst nicht an:
//! Mit einem Besitzer schaltet das Portal auf den GNOME-Weg um und erwartet
//! dort auch die Sitzungsverwaltung von gnome-session (Client-Registrierung,
//! QueryEndSession).
//!
//! Methoden wie gnome-screensaver und KDE:
//! - `Inhibit(s Anwendung, s Grund) → u Cookie`, `UnInhibit(u Cookie)`:
//!   Solange ein Cookie gilt, sperrt der Leerlauf nicht. Jedes Cookie gehört
//!   dem Absender (eindeutiger Busname), der es geholt hat; nur er hebt es
//!   auf. Verlässt der Absender den Bus (Prozessende), fallen seine Cookies
//!   weg.
//! - `Lock()`, `SetActive(true)`: sperrt sofort (`--sperren --sofort`).
//!   `SetActive(false)` entsperrt nicht (das geht nur mit Passwort) und
//!   liefert `false`.
//! - `GetActive`, `GetActiveTime`, Signal `ActiveChanged(b)`: ob der
//!   Sperrbildschirm läuft, auch einer, den das Tastenkürzel gestartet hat.
//! - `SimulateUserActivity()`: zählt den Leerlauf von vorn.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

pub const NAME: &str = "org.freedesktop.ScreenSaver";
pub const SCHNITTSTELLE: &str = "org.freedesktop.ScreenSaver";
/// Beide üblichen Objektpfade: Firefox ruft `/ScreenSaver`, Chromium und
/// das Portal `/org/freedesktop/ScreenSaver`.
pub const PFADE: [&str; 2] = ["/org/freedesktop/ScreenSaver", "/ScreenSaver"];

/// Mehr Hemmungen nimmt ein einzelner Absender nicht an (Schutz vor einem
/// Programm, das Cookies holt und nie zurückgibt).
pub const HOECHSTENS_JE_ABSENDER: usize = 64;

pub const XML: &str = r#"<node>
  <interface name="org.freedesktop.ScreenSaver">
    <method name="Inhibit">
      <arg type="s" name="application_name" direction="in"/>
      <arg type="s" name="reason_for_inhibit" direction="in"/>
      <arg type="u" name="cookie" direction="out"/>
    </method>
    <method name="UnInhibit">
      <arg type="u" name="cookie" direction="in"/>
    </method>
    <method name="Lock"/>
    <method name="SimulateUserActivity"/>
    <method name="GetActive">
      <arg type="b" name="active" direction="out"/>
    </method>
    <method name="GetActiveTime">
      <arg type="u" name="seconds" direction="out"/>
    </method>
    <method name="SetActive">
      <arg type="b" name="active" direction="in"/>
      <arg type="b" name="result" direction="out"/>
    </method>
    <signal name="ActiveChanged">
      <arg type="b" name="active"/>
    </signal>
  </interface>
</node>"#;

/// Beobachtete Absender und wie ihre Beobachtung endet.
type Beobachtet = BTreeMap<String, Box<dyn FnOnce()>>;

/// Eine gültige Hemmung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hemmung {
    pub absender: String,
    pub anwendung: String,
    pub grund: String,
}

/// Was beim Hemmen schiefging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HemmFehler {
    /// [`HOECHSTENS_JE_ABSENDER`] erreicht.
    ZuViele,
}

/// Was `UnInhibit` bewirkt hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aufgehoben {
    /// Cookie entfernt; `rest` Hemmungen gelten noch.
    Ja { rest: usize },
    /// Kein solches Cookie (schon aufgehoben oder nie vergeben).
    Unbekannt,
    /// Das Cookie gehört einem anderen Absender; es bleibt.
    Fremd,
}

/// Die Hemmungen der Sitzung, je Cookie. Rein, ohne D-Bus (Unit-Tests).
#[derive(Debug, Default)]
pub struct Hemmungen {
    letztes: u32,
    eintraege: BTreeMap<u32, Hemmung>,
}

impl Hemmungen {
    /// Nimmt eine Hemmung von `absender` an und liefert ihr Cookie (nie 0,
    /// nie ein noch gültiges).
    pub fn hemmen(
        &mut self,
        absender: &str,
        anwendung: &str,
        grund: &str,
    ) -> Result<u32, HemmFehler> {
        if self.von(absender) >= HOECHSTENS_JE_ABSENDER {
            return Err(HemmFehler::ZuViele);
        }
        let mut cookie = self.letztes;
        loop {
            cookie = cookie.wrapping_add(1);
            if cookie != 0 && !self.eintraege.contains_key(&cookie) {
                break;
            }
        }
        self.letztes = cookie;
        self.eintraege.insert(
            cookie,
            Hemmung {
                absender: absender.to_string(),
                anwendung: anwendung.to_string(),
                grund: grund.to_string(),
            },
        );
        Ok(cookie)
    }

    /// Hebt `cookie` auf, wenn es `absender` gehört.
    pub fn aufheben(&mut self, absender: &str, cookie: u32) -> Aufgehoben {
        match self.eintraege.get(&cookie) {
            None => Aufgehoben::Unbekannt,
            Some(h) if h.absender != absender => Aufgehoben::Fremd,
            Some(_) => {
                self.eintraege.remove(&cookie);
                Aufgehoben::Ja {
                    rest: self.eintraege.len(),
                }
            }
        }
    }

    /// Entfernt alle Hemmungen von `absender` (er hat den Bus verlassen) und
    /// liefert ihre Cookies.
    pub fn absender_weg(&mut self, absender: &str) -> Vec<u32> {
        let weg: Vec<u32> = self
            .eintraege
            .iter()
            .filter(|(_, h)| h.absender == absender)
            .map(|(c, _)| *c)
            .collect();
        for c in &weg {
            self.eintraege.remove(c);
        }
        weg
    }

    /// Anzahl der Hemmungen von `absender`.
    pub fn von(&self, absender: &str) -> usize {
        self.eintraege
            .values()
            .filter(|h| h.absender == absender)
            .count()
    }

    pub fn anzahl(&self) -> usize {
        self.eintraege.len()
    }

    pub fn aktiv(&self) -> bool {
        !self.eintraege.is_empty()
    }

    /// „Firefox (video-playing), …“ für das Protokoll.
    pub fn beschreibung(&self) -> String {
        self.eintraege
            .values()
            .map(|h| format!("{} ({})", h.anwendung, h.grund))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Was der Leerlaufdienst dem D-Bus-Teil anbietet.
pub trait Sitzung {
    /// Neue Hemmung, schon eingetragen; `erste`: vorher galt keine.
    fn gehemmt(&mut self, erste: bool);
    /// Die letzte Hemmung ist weg: Leerlauf von vorn zählen.
    fn frei(&mut self);
    /// Sofort sperren (`Lock`, `SetActive(true)`); `false`, wenn nicht
    /// startbar.
    fn sperren(&mut self, anlass: &str) -> bool;
    /// Ob der Sperrbildschirm läuft.
    fn gesperrt(&mut self) -> bool;
    /// Sekunden, seit der Sperrbildschirm läuft (0 ohne Sperre).
    fn gesperrt_seit(&mut self) -> u32;
    /// Benutzeraktivität: Leerlauf von vorn zählen.
    fn aktivitaet(&mut self);
    fn hemmungen(&mut self) -> &mut Hemmungen;
}

fn protokoll(text: &str) {
    crate::aktion::protokoll(&format!("ScreenSaver: {text}"));
}

/// Sendet `ActiveChanged(aktiv)` an beiden Pfaden.
pub fn aktiv_melden(bus: &gio::DBusConnection, aktiv: bool) {
    for pfad in PFADE {
        if let Err(e) = bus.emit_signal(
            None,
            pfad,
            SCHNITTSTELLE,
            "ActiveChanged",
            Some(&(aktiv,).to_variant()),
        ) {
            protokoll(&format!("ActiveChanged an {pfad}: {e}"));
        }
    }
}

/// Beobachtet `absender` und räumt seine Cookies ab, sobald er den Bus
/// verlässt; eine Beobachtung je Absender.
fn beobachten<S: Sitzung + 'static>(
    bus: &gio::DBusConnection,
    sitzung: &Rc<RefCell<S>>,
    beobachtet: &Rc<RefCell<Beobachtet>>,
    absender: &str,
) {
    if beobachtet.borrow().contains_key(absender) {
        return;
    }
    let s = sitzung.clone();
    let b = beobachtet.clone();
    let id = gio::bus_watch_name_on_connection(
        bus,
        absender,
        gio::BusNameWatcherFlags::NONE,
        |_, _, _| {},
        move |_, name| {
            {
                let mut s = s.borrow_mut();
                let weg = s.hemmungen().absender_weg(name);
                if !weg.is_empty() {
                    let rest = s.hemmungen().anzahl();
                    protokoll(&format!(
                        "{name} hat den Bus verlassen, Cookie(s) {weg:?} aufgehoben, {rest} gelten noch"
                    ));
                    if rest == 0 {
                        s.frei();
                    }
                }
            }
            // Die Beobachtung selbst erst nach dem Rückruf beenden.
            if let Some(beenden) = b.borrow_mut().remove(name) {
                glib::idle_add_local_once(beenden);
            }
        },
    );
    beobachtet.borrow_mut().insert(
        absender.to_string(),
        Box::new(move || gio::bus_unwatch_name(id)),
    );
}

fn methode<S: Sitzung + 'static>(
    bus: &gio::DBusConnection,
    sitzung: &Rc<RefCell<S>>,
    beobachtet: &Rc<RefCell<Beobachtet>>,
    absender: &str,
    name: &str,
    parameter: &glib::Variant,
    invocation: gio::DBusMethodInvocation,
) {
    match name {
        "Inhibit" => {
            let (anwendung, grund) = parameter.get::<(String, String)>().unwrap_or_default();
            let ergebnis = {
                let mut s = sitzung.borrow_mut();
                let erste = !s.hemmungen().aktiv();
                let r = s.hemmungen().hemmen(absender, &anwendung, &grund);
                if r.is_ok() {
                    s.gehemmt(erste);
                }
                r.map(|c| (c, s.hemmungen().anzahl()))
            };
            match ergebnis {
                Ok((cookie, anzahl)) => {
                    protokoll(&format!(
                        "Inhibit von {absender} ({anwendung}: {grund}) → Cookie {cookie}, {anzahl} gelten"
                    ));
                    beobachten(bus, sitzung, beobachtet, absender);
                    invocation.return_value(Some(&(cookie,).to_variant()));
                }
                Err(HemmFehler::ZuViele) => {
                    protokoll(&format!(
                        "Inhibit von {absender} abgelehnt: schon {HOECHSTENS_JE_ABSENDER} Hemmungen"
                    ));
                    invocation.return_dbus_error(
                        "org.freedesktop.DBus.Error.LimitsExceeded",
                        "zu viele Hemmungen dieses Absenders",
                    );
                }
            }
        }
        "UnInhibit" => {
            let (cookie,) = parameter.get::<(u32,)>().unwrap_or_default();
            let mut s = sitzung.borrow_mut();
            match s.hemmungen().aufheben(absender, cookie) {
                Aufgehoben::Ja { rest } => {
                    protokoll(&format!(
                        "UnInhibit {cookie} von {absender}, {rest} gelten noch"
                    ));
                    if rest == 0 {
                        s.frei();
                    }
                }
                Aufgehoben::Unbekannt => {
                    protokoll(&format!("UnInhibit {cookie} von {absender}: unbekannt"))
                }
                Aufgehoben::Fremd => protokoll(&format!(
                    "UnInhibit {cookie} von {absender}: gehört einem anderen Absender, bleibt"
                )),
            }
            invocation.return_value(None);
        }
        "Lock" => {
            protokoll(&format!("Lock von {absender}"));
            sitzung.borrow_mut().sperren("Lock");
            invocation.return_value(None);
        }
        "SetActive" => {
            let (an,) = parameter.get::<(bool,)>().unwrap_or_default();
            protokoll(&format!("SetActive({an}) von {absender}"));
            let ergebnis = if an {
                sitzung.borrow_mut().sperren("SetActive")
            } else {
                // Entsperren geht nur mit dem Passwort am Sperrbildschirm.
                false
            };
            invocation.return_value(Some(&(ergebnis,).to_variant()));
        }
        "GetActive" => {
            let aktiv = sitzung.borrow_mut().gesperrt();
            invocation.return_value(Some(&(aktiv,).to_variant()));
        }
        "GetActiveTime" => {
            let sekunden = sitzung.borrow_mut().gesperrt_seit();
            invocation.return_value(Some(&(sekunden,).to_variant()));
        }
        "SimulateUserActivity" => {
            sitzung.borrow_mut().aktivitaet();
            invocation.return_value(None);
        }
        _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", name),
    }
}

/// Meldet die Schnittstelle an beiden Pfaden an und bewirbt sich um
/// [`NAME`]. Fehler protokolliert er nur: Der Leerlaufdienst läuft dann
/// ohne D-Bus weiter wie bisher. Liefert die Busverbindung (für
/// `ActiveChanged`).
pub fn anbieten<S: Sitzung + 'static>(sitzung: Rc<RefCell<S>>) -> Option<gio::DBusConnection> {
    let bus = match gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
        Ok(bus) => bus,
        Err(e) => {
            protokoll(&format!("kein Sitzungsbus, Dienst nicht angeboten: {e}"));
            return None;
        }
    };
    let info = gio::DBusNodeInfo::for_xml(XML).ok()?;
    let iface = info.lookup_interface(SCHNITTSTELLE)?;
    let beobachtet = Rc::new(RefCell::new(Beobachtet::new()));
    for pfad in PFADE {
        let s = sitzung.clone();
        let b = beobachtet.clone();
        let ergebnis = bus
            .register_object(pfad, &iface)
            .method_call(move |bus, absender, _, _, name, parameter, invocation| {
                methode(
                    &bus,
                    &s,
                    &b,
                    absender.unwrap_or(""),
                    name,
                    &parameter,
                    invocation,
                )
            })
            .build();
        if let Err(e) = ergebnis {
            protokoll(&format!("Objekt {pfad} nicht anmeldbar: {e}"));
            return None;
        }
    }
    // Ohne Warteschlange und ohne Ersetzen: Hat schon ein anderer Dienst
    // den Namen, bleibt er bei ihm.
    // Der Name gilt bis zum Ende des Dienstes (die Kennung wird nie
    // freigegeben).
    let _ = gio::bus_own_name_on_connection(
        &bus,
        NAME,
        gio::BusNameOwnerFlags::DO_NOT_QUEUE,
        |_, name| protokoll(&format!("{name} auf dem Sitzungsbus übernommen")),
        |_, name| {
            protokoll(&format!(
                "{name} nicht erhalten oder verloren (anderer Besitzer?)"
            ))
        },
    );
    Some(bus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies_je_absender_und_aufheben() {
        let mut h = Hemmungen::default();
        assert!(!h.aktiv());
        let a = h.hemmen(":1.10", "Firefox", "video-playing").unwrap();
        let b = h.hemmen(":1.11", "mpv", "Wiedergabe").unwrap();
        assert_ne!(a, 0);
        assert_ne!(a, b);
        assert_eq!(h.anzahl(), 2);
        assert!(h.beschreibung().contains("Firefox (video-playing)"));
        // fremdes Cookie bleibt
        assert_eq!(h.aufheben(":1.11", a), Aufgehoben::Fremd);
        assert_eq!(h.anzahl(), 2);
        assert_eq!(h.aufheben(":1.10", a), Aufgehoben::Ja { rest: 1 });
        assert_eq!(h.aufheben(":1.10", a), Aufgehoben::Unbekannt);
        assert_eq!(h.aufheben(":1.11", b), Aufgehoben::Ja { rest: 0 });
        assert!(!h.aktiv());
    }

    #[test]
    fn prozessende_raeumt_auf() {
        let mut h = Hemmungen::default();
        let a1 = h.hemmen(":1.20", "Portal", "Inhibit").unwrap();
        let a2 = h.hemmen(":1.20", "Portal", "Inhibit").unwrap();
        let b = h.hemmen(":1.21", "Chromium", "Video").unwrap();
        assert_eq!(h.von(":1.20"), 2);
        assert_eq!(h.absender_weg(":1.20"), vec![a1, a2]);
        assert_eq!(h.anzahl(), 1);
        assert!(h.aktiv());
        assert!(h.absender_weg(":1.20").is_empty());
        assert_eq!(h.aufheben(":1.21", b), Aufgehoben::Ja { rest: 0 });
    }

    #[test]
    fn hoechstzahl_je_absender() {
        let mut h = Hemmungen::default();
        for _ in 0..HOECHSTENS_JE_ABSENDER {
            h.hemmen(":1.30", "x", "y").unwrap();
        }
        assert_eq!(h.hemmen(":1.30", "x", "y"), Err(HemmFehler::ZuViele));
        // andere Absender sind nicht betroffen
        assert!(h.hemmen(":1.31", "x", "y").is_ok());
    }

    #[test]
    fn cookie_nie_null_und_nie_doppelt() {
        let mut h = Hemmungen {
            letztes: u32::MAX - 1,
            ..Default::default()
        };
        let a = h.hemmen(":1.40", "x", "y").unwrap();
        assert_eq!(a, u32::MAX);
        let b = h.hemmen(":1.40", "x", "y").unwrap();
        assert_eq!(b, 1, "überspringt 0");
        // 2 ist noch vergeben → übersprungen
        h.eintraege.insert(
            2,
            Hemmung {
                absender: ":1.41".into(),
                anwendung: String::new(),
                grund: String::new(),
            },
        );
        h.letztes = 1;
        assert_eq!(h.hemmen(":1.40", "x", "y").unwrap(), 3);
    }

    #[test]
    fn schnittstelle_ist_gueltiges_xml() {
        let info = gio::DBusNodeInfo::for_xml(XML).expect("XML");
        let iface = info.lookup_interface(SCHNITTSTELLE).expect("Schnittstelle");
        for m in [
            "Inhibit",
            "UnInhibit",
            "Lock",
            "GetActive",
            "GetActiveTime",
            "SetActive",
            "SimulateUserActivity",
        ] {
            assert!(iface.lookup_method(m).is_some(), "{m}");
        }
        assert!(iface.lookup_signal("ActiveChanged").is_some());
    }
}
