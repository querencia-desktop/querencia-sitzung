//! Querencia polkit-Agent: Legitimierungsdialog im klassischen Stil für die
//! Querencia-Sitzung (G2-K, Ergänzung nach Befund G2-I: polkitd läuft, aber
//! kein Agent ist installiert).
//!
//! Registriert sich bei polkitd über D-Bus
//! (`org.freedesktop.PolicyKit1.Authority.RegisterAuthenticationAgent`) mit
//! dem Prozess als Subjekt (polkitd ordnet ihn seiner logind-Sitzung zu) und
//! stellt das Objekt `/org/querencia/PolkitAgent` mit der Schnittstelle
//! `org.freedesktop.PolicyKit1.AuthenticationAgent` bereit. Die
//! Passwortprüfung übernimmt `PolkitAgentSession` aus libpolkit-agent-1 (per
//! `dlopen`, keine Entwicklungsdateien nötig), die wie bei lxpolkit mit dem
//! Helfer `polkit-agent-helper-1` spricht (unter polkit 127 über den
//! systemd-Socket `polkit-agent-helper.socket`); der Agent prüft nie selbst
//! gegen PAM.
//!
//! Startet polkitd neu (z. B. needrestart nach einem Update, QD-156), kennt
//! der neue polkitd den Agenten nicht. Der Agent beobachtet darum auf dem
//! System-Bus `NameOwnerChanged` für `org.freedesktop.PolicyKit1` und
//! registriert sich beim neuen Besitzer erneut, mit demselben Subjekt und
//! derselben Locale wie beim Start; antwortet der neue polkitd noch nicht,
//! wiederholt er mit wachsender Wartezeit (0.1.8).
//!
//! Aufruf: `querencia-polkit-agent` (läuft bis zum Sitzungsende; für den
//! Autostart), `--version`, `--hilfe`.

// Dunkelmodus beim Start (QD-109), wie die Dialoge von querencia-sitzung.
#[path = "../farbschema.rs"]
mod farbschema;
// Übersetzungen: dasselbe Modul und dieselbe Domäne wie querencia-sitzung
// (beide Programme liegen in derselben Ablage).
#[allow(dead_code)]
#[path = "../i18n.rs"]
mod i18n;

use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_char, c_int, c_void};
use std::rc::Rc;

use glib::translate::FromGlibPtrFull;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

const APP_ID: &str = "org.querencia.PolkitAgent";
const OBJEKT: &str = "/org/querencia/PolkitAgent";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_VERSUCHE: u32 = 3;
const POLKIT_NAME: &str = "org.freedesktop.PolicyKit1";
const POLKIT_PFAD: &str = "/org/freedesktop/PolicyKit1/Authority";
const POLKIT_SCHNITTSTELLE: &str = "org.freedesktop.PolicyKit1.Authority";
/// Zeitgrenze eines Registrierungsversuchs beim neuen polkitd.
const NEU_ZEITGRENZE_MS: i32 = 5000;
/// Wartezeit vor dem 2., 3., … Versuch, falls der neue polkitd noch nicht
/// antwortet (zusammen 3 min Wartezeit). Danach ruht der Agent bis zum nächsten
/// Besitzerwechsel.
const WIEDERHOLUNG_MS: [u64; 11] = [
    500, 1000, 2000, 4000, 8000, 15000, 30000, 30000, 30000, 30000, 30000,
];
const SCHNITTSTELLE: &str = r#"
<node>
  <interface name="org.freedesktop.PolicyKit1.AuthenticationAgent">
    <method name="BeginAuthentication">
      <arg type="s" name="action_id" direction="in"/>
      <arg type="s" name="message" direction="in"/>
      <arg type="s" name="icon_name" direction="in"/>
      <arg type="a{ss}" name="details" direction="in"/>
      <arg type="s" name="cookie" direction="in"/>
      <arg type="a(sa{sv})" name="identities" direction="in"/>
    </method>
    <method name="CancelAuthentication">
      <arg type="s" name="cookie" direction="in"/>
    </method>
  </interface>
</node>
"#;

fn protokoll(text: &str) {
    eprintln!("querencia-polkit-agent: {text}");
}

// ------------------------------------------------------------ libpolkit

type UnixUserNew = unsafe extern "C" fn(c_int) -> *mut glib::gobject_ffi::GObject;
type SessionNew = unsafe extern "C" fn(
    *mut glib::gobject_ffi::GObject,
    *const c_char,
) -> *mut glib::gobject_ffi::GObject;
type SessionFn = unsafe extern "C" fn(*mut glib::gobject_ffi::GObject);
type SessionResponse = unsafe extern "C" fn(*mut glib::gobject_ffi::GObject, *const c_char);

/// Die vier Funktionen von libpolkit-agent-1 und libpolkit-gobject-1, zur
/// Laufzeit geladen.
struct Polkit {
    _agent: *mut c_void,
    _gobject: *mut c_void,
    unix_user_new: UnixUserNew,
    session_new: SessionNew,
    session_initiate: SessionFn,
    session_cancel: SessionFn,
    session_response: SessionResponse,
}

impl Polkit {
    fn laden() -> Result<Rc<Polkit>, String> {
        // SAFETY: dlopen/dlsym mit gültigen C-Strings; die Signaturen
        // entsprechen polkitagentsession.h und polkitunixuser.h.
        unsafe {
            let oeffnen = |name: &std::ffi::CStr| -> Result<*mut c_void, String> {
                let h = libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
                if h.is_null() {
                    Err(format!("{} nicht ladbar", name.to_string_lossy()))
                } else {
                    Ok(h)
                }
            };
            let gobject = oeffnen(c"libpolkit-gobject-1.so.0")?;
            let agent = oeffnen(c"libpolkit-agent-1.so.0")?;
            let symbol = |h: *mut c_void, s: &std::ffi::CStr| -> Result<*mut c_void, String> {
                let p = libc::dlsym(h, s.as_ptr());
                if p.is_null() {
                    Err(format!("{} fehlt", s.to_string_lossy()))
                } else {
                    Ok(p)
                }
            };
            Ok(Rc::new(Polkit {
                _agent: agent,
                _gobject: gobject,
                unix_user_new: std::mem::transmute::<*mut c_void, UnixUserNew>(symbol(
                    gobject,
                    c"polkit_unix_user_new",
                )?),
                session_new: std::mem::transmute::<*mut c_void, SessionNew>(symbol(
                    agent,
                    c"polkit_agent_session_new",
                )?),
                session_initiate: std::mem::transmute::<*mut c_void, SessionFn>(symbol(
                    agent,
                    c"polkit_agent_session_initiate",
                )?),
                session_cancel: std::mem::transmute::<*mut c_void, SessionFn>(symbol(
                    agent,
                    c"polkit_agent_session_cancel",
                )?),
                session_response: std::mem::transmute::<*mut c_void, SessionResponse>(symbol(
                    agent,
                    c"polkit_agent_session_response",
                )?),
            }))
        }
    }

    /// Neue Helfer-Sitzung für `uid` und `cookie` (noch nicht gestartet).
    fn session(&self, uid: u32, cookie: &str) -> Result<glib::Object, String> {
        let cookie = CString::new(cookie).map_err(|_| "Cookie mit Nullzeichen".to_string())?;
        // SAFETY: polkit_unix_user_new liefert eine neue Referenz, die
        // polkit_agent_session_new selbst referenziert; danach geben wir
        // unsere frei. Das Session-Objekt gehört uns (volle Referenz).
        unsafe {
            let identity = (self.unix_user_new)(uid as c_int);
            if identity.is_null() {
                return Err("polkit_unix_user_new fehlgeschlagen".to_string());
            }
            let session = (self.session_new)(identity, cookie.as_ptr());
            glib::gobject_ffi::g_object_unref(identity);
            if session.is_null() {
                return Err("polkit_agent_session_new fehlgeschlagen".to_string());
            }
            Ok(glib::Object::from_glib_full(session))
        }
    }

    fn initiate(&self, session: &glib::Object) {
        // SAFETY: gültiges PolkitAgentSession-Objekt.
        unsafe { (self.session_initiate)(session.as_ptr()) }
    }

    fn cancel(&self, session: &glib::Object) {
        // SAFETY: gültiges PolkitAgentSession-Objekt.
        unsafe { (self.session_cancel)(session.as_ptr()) }
    }

    fn response(&self, session: &glib::Object, antwort: &str) {
        let Ok(antwort) = CString::new(antwort) else {
            return;
        };
        // SAFETY: gültiges Objekt, gültiger C-String.
        unsafe { (self.session_response)(session.as_ptr(), antwort.as_ptr()) }
    }
}

// ------------------------------------------------------------ Anfrage

/// Eine offene Legitimierungsanfrage von polkitd.
struct Anfrage {
    cookie: String,
    uid: u32,
    benutzer: String,
    invocation: Option<gio::DBusMethodInvocation>,
    window: gtk::Window,
    eingabe: gtk::PasswordEntry,
    status: gtk::Label,
    knopf: gtk::Button,
    session: Option<glib::Object>,
    versuche: u32,
    wartet_auf_antwort: bool,
}

thread_local! {
    static OFFEN: RefCell<Option<Rc<RefCell<Anfrage>>>> = const { RefCell::new(None) };
}

/// Wählt die Identität, für die das Passwort gefragt wird: der eigene
/// Benutzer, wenn polkitd ihn anbietet, sonst der erste Unix-Benutzer.
pub fn identitaet_waehlen(identities: &[(String, u32)], eigene_uid: u32) -> Option<u32> {
    identities
        .iter()
        .find(|(art, uid)| art == "unix-user" && *uid == eigene_uid)
        .or_else(|| identities.iter().find(|(art, _)| art == "unix-user"))
        .map(|(_, uid)| *uid)
}

fn benutzername(uid: u32) -> String {
    // SAFETY: getpwuid liefert einen statischen Eintrag oder NULL.
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() || (*pw).pw_name.is_null() {
            return format!("uid {uid}");
        }
        std::ffi::CStr::from_ptr((*pw).pw_name)
            .to_string_lossy()
            .into_owned()
    }
}

fn knopf(beschriftung: &str, symbole: &[&str]) -> gtk::Button {
    let button = gtk::Button::new();
    let inhalt = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let bild = gtk::Image::from_gicon(&gio::ThemedIcon::from_names(symbole));
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

/// Baut das Dialogfenster; `aktion`/`nachricht`/`symbol` von polkitd.
fn fenster(
    app: &gtk::Application,
    aktion: &str,
    nachricht: &str,
    symbol: &str,
    benutzer: &str,
) -> (
    gtk::Window,
    gtk::PasswordEntry,
    gtk::Label,
    gtk::Button,
    gtk::Button,
) {
    let window = gtk::Window::builder()
        .application(app)
        .title(i18n::gettext("Authenticate"))
        .resizable(false)
        .build();
    window.add_css_class("qs-dialog");
    let symbole: Vec<&str> = if symbol.is_empty() {
        vec!["dialog-password", "dialog-authentication"]
    } else {
        vec![symbol, "dialog-password"]
    };
    let bild = gtk::Image::from_gicon(&gio::ThemedIcon::from_names(&symbole));
    bild.set_pixel_size(48);
    bild.set_valign(gtk::Align::Start);
    let frage = gtk::Label::builder()
        .label(format!(
            "<b><big>{}</big></b>",
            glib::markup_escape_text(&if nachricht.is_empty() {
                // TRANSLATORS: nur wenn polkitd keine eigene Meldung schickt.
                i18n::gettext("An application requires authentication.")
            } else {
                nachricht.to_string()
            })
        ))
        .use_markup(true)
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(48)
        .build();
    let detail = gtk::Label::builder()
        .label(i18n::gettext_f("Action: {action}", &[("action", aktion)]))
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(48)
        .build();
    detail.add_css_class("dim-label");
    let hinweis = gtk::Label::builder()
        .label(i18n::gettext_f(
            "Password for {user}:",
            &[("user", benutzer)],
        ))
        .xalign(0.0)
        .build();
    let eingabe = gtk::PasswordEntry::builder()
        .show_peek_icon(true)
        .activates_default(true)
        .sensitive(false)
        .build();
    let status = gtk::Label::builder().xalign(0.0).wrap(true).build();
    status.set_label(&i18n::gettext("Connecting to the authentication service…"));
    status.add_css_class("dim-label");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 8);
    text.append(&frage);
    text.append(&detail);
    text.append(&hinweis);
    text.append(&eingabe);
    text.append(&status);
    let oben = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    oben.append(&bild);
    oben.append(&text);
    let abbrechen = knopf(
        &i18n::gettext("_Cancel"),
        &["process-stop", "dialog-cancel"],
    );
    let legitimieren = knopf(
        &i18n::gettext("_Authenticate"),
        &["dialog-password", "emblem-ok"],
    );
    legitimieren.add_css_class("suggested-action");
    legitimieren.set_sensitive(false);
    let knoepfe = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    knoepfe.set_halign(gtk::Align::End);
    knoepfe.append(&abbrechen);
    knoepfe.append(&legitimieren);
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
    window.set_default_widget(Some(&legitimieren));
    (window, eingabe, status, legitimieren, abbrechen)
}

/// Beendet die offene Anfrage: `Ok(())` an polkitd oder ein D-Bus-Fehler.
fn abschliessen(
    polkit: &Polkit,
    anfrage: &Rc<RefCell<Anfrage>>,
    ergebnis: Result<(), (&str, String)>,
) {
    // Erst alles aus der Anfrage nehmen und den Borrow freigeben:
    // `polkit_agent_session_cancel` sendet `completed` synchron, und der
    // Signalhandler greift selbst auf die Anfrage zu.
    let (session, wartet, invocation, window) = {
        let mut a = anfrage.borrow_mut();
        (
            a.session.take(),
            a.wartet_auf_antwort,
            a.invocation.take(),
            a.window.clone(),
        )
    };
    if let Some(invocation) = invocation {
        match ergebnis {
            Ok(()) => invocation.return_value(None),
            Err((name, text)) => invocation.return_dbus_error(name, &text),
        }
    }
    OFFEN.with(|o| {
        if o.borrow().as_ref().is_some_and(|x| Rc::ptr_eq(x, anfrage)) {
            *o.borrow_mut() = None;
        }
    });
    if let Some(session) = session
        && wartet
    {
        polkit.cancel(&session);
    }
    window.close();
}

fn abbrechen(polkit: &Polkit, anfrage: &Rc<RefCell<Anfrage>>, grund: &str) {
    protokoll(&format!(
        "abgebrochen: {grund} (Cookie {})",
        anfrage.borrow().cookie
    ));
    abschliessen(
        polkit,
        anfrage,
        Err((
            "org.freedesktop.PolicyKit1.Error.Cancelled",
            format!("Legitimierung abgebrochen: {grund}"),
        )),
    );
}

/// Startet eine (neue) Helfer-Sitzung für die Anfrage und verdrahtet die
/// Signale `request`, `show-error`, `show-info`, `completed`.
fn sitzung_starten(polkit: &Rc<Polkit>, anfrage: &Rc<RefCell<Anfrage>>) {
    let (uid, cookie) = {
        let a = anfrage.borrow();
        (a.uid, a.cookie.clone())
    };
    let session = match polkit.session(uid, &cookie) {
        Ok(s) => s,
        Err(e) => {
            protokoll(&format!("Helfer-Sitzung nicht anlegbar: {e}"));
            abschliessen(
                polkit,
                anfrage,
                Err(("org.freedesktop.PolicyKit1.Error.Failed", e)),
            );
            return;
        }
    };
    let eingabe = anfrage.borrow().eingabe.clone();
    let status = anfrage.borrow().status.clone();
    let legitimieren = anfrage.borrow().knopf.clone();
    session.connect_local("request", false, {
        let anfrage = anfrage.clone();
        let eingabe = eingabe.clone();
        let status = status.clone();
        let legitimieren = legitimieren.clone();
        move |werte| {
            let frage = werte
                .get(1)
                .and_then(|v| v.get::<String>().ok())
                .unwrap_or_default();
            let echo = werte
                .get(2)
                .and_then(|v| v.get::<bool>().ok())
                .unwrap_or(false);
            protokoll(&format!("Helfer fragt: {} (Echo {echo})", frage.trim()));
            anfrage.borrow_mut().wartet_auf_antwort = true;
            eingabe.set_sensitive(true);
            legitimieren.set_sensitive(true);
            eingabe.grab_focus();
            status.set_label(&i18n::gettext("Enter the password and press Enter"));
            None
        }
    });
    session.connect_local("show-error", false, {
        let status = status.clone();
        move |werte| {
            let text = werte
                .get(1)
                .and_then(|v| v.get::<String>().ok())
                .unwrap_or_default();
            protokoll(&format!("Helfer-Fehler: {text}"));
            status.set_label(&text);
            None
        }
    });
    session.connect_local("show-info", false, {
        let status = status.clone();
        move |werte| {
            let text = werte
                .get(1)
                .and_then(|v| v.get::<String>().ok())
                .unwrap_or_default();
            protokoll(&format!("Helfer-Hinweis: {text}"));
            status.set_label(&text);
            None
        }
    });
    session.connect_local("completed", false, {
        let anfrage = anfrage.clone();
        let polkit = polkit.clone();
        move |werte| {
            let erfolg = werte
                .get(1)
                .and_then(|v| v.get::<bool>().ok())
                .unwrap_or(false);
            // Nach `completed` ist die Sitzung verbraucht. Eine schon
            // abgeschlossene Anfrage (Abbruch) braucht nichts mehr.
            {
                let Ok(mut a) = anfrage.try_borrow_mut() else {
                    return None;
                };
                a.invocation.as_ref()?;
                a.session = None;
                a.wartet_auf_antwort = false;
            }
            if erfolg {
                protokoll(&format!(
                    "Legitimierung erfolgreich (Cookie {})",
                    anfrage.borrow().cookie
                ));
                abschliessen(&polkit, &anfrage, Ok(()));
                return None;
            }
            let versuche = {
                let mut a = anfrage.borrow_mut();
                a.versuche += 1;
                a.versuche
            };
            protokoll(&format!(
                "Legitimierung fehlgeschlagen, Versuch {versuche} von {MAX_VERSUCHE}"
            ));
            if versuche >= MAX_VERSUCHE {
                abschliessen(
                    &polkit,
                    &anfrage,
                    Err((
                        "org.freedesktop.PolicyKit1.Error.Failed",
                        "Zu viele Fehlversuche".to_string(),
                    )),
                );
                return None;
            }
            let (status, eingabe) = {
                let a = anfrage.borrow();
                (a.status.clone(), a.eingabe.clone())
            };
            status.set_label(&i18n::gettext_f(
                "Wrong password, please try again ({attempt} of {max})",
                &[
                    ("attempt", &versuche.to_string()),
                    ("max", &MAX_VERSUCHE.to_string()),
                ],
            ));
            eingabe.set_text("");
            // Neue Sitzung im nächsten Leerlauf, nicht aus dem Signal heraus.
            let anfrage2 = anfrage.clone();
            let polkit2 = polkit.clone();
            glib::idle_add_local_once(move || {
                if OFFEN.with(|o| {
                    o.borrow()
                        .as_ref()
                        .is_some_and(|x| Rc::ptr_eq(x, &anfrage2))
                }) {
                    sitzung_starten(&polkit2, &anfrage2);
                }
            });
            None
        }
    });
    anfrage.borrow_mut().session = Some(session.clone());
    polkit.initiate(&session);
}

/// Antwort des Benutzers (Enter oder „Legitimieren“).
fn antworten(polkit: &Polkit, anfrage: &Rc<RefCell<Anfrage>>) {
    let (session, text, wartet) = {
        let a = anfrage.borrow();
        (
            a.session.clone(),
            a.eingabe.text().to_string(),
            a.wartet_auf_antwort,
        )
    };
    let (Some(session), true) = (session, wartet) else {
        return;
    };
    {
        let mut a = anfrage.borrow_mut();
        a.wartet_auf_antwort = false;
        a.eingabe.set_sensitive(false);
        a.knopf.set_sensitive(false);
        a.status.set_label(&i18n::gettext("Checking the password…"));
        a.eingabe.set_text("");
    }
    polkit.response(&session, &text);
}

/// `BeginAuthentication` von polkitd.
fn beginnen(
    app: &gtk::Application,
    polkit: &Rc<Polkit>,
    parameter: &glib::Variant,
    invocation: gio::DBusMethodInvocation,
) {
    type Param = (
        String,
        String,
        String,
        std::collections::HashMap<String, String>,
        String,
        Vec<(String, std::collections::HashMap<String, glib::Variant>)>,
    );
    let Some((aktion, nachricht, symbol, _details, cookie, identities)) = parameter.get::<Param>()
    else {
        invocation.return_dbus_error(
            "org.freedesktop.PolicyKit1.Error.Failed",
            "unerwartete Parameter",
        );
        return;
    };
    if OFFEN.with(|o| o.borrow().is_some()) {
        protokoll(&format!(
            "Anfrage für {aktion} abgewiesen: es ist schon eine offen"
        ));
        invocation.return_dbus_error(
            "org.freedesktop.PolicyKit1.Error.Failed",
            "Es ist bereits eine Legitimierung offen",
        );
        return;
    }
    let liste: Vec<(String, u32)> = identities
        .iter()
        .map(|(art, werte)| {
            (
                art.clone(),
                werte
                    .get("uid")
                    .and_then(|v| v.get::<u32>())
                    .unwrap_or(u32::MAX),
            )
        })
        .collect();
    // SAFETY: getuid hat keine Vorbedingungen.
    let eigene = unsafe { libc::getuid() };
    let Some(uid) = identitaet_waehlen(&liste, eigene) else {
        protokoll(&format!(
            "Anfrage für {aktion}: keine Unix-Benutzeridentität ({liste:?})"
        ));
        invocation.return_dbus_error(
            "org.freedesktop.PolicyKit1.Error.Failed",
            "Keine passende Identität",
        );
        return;
    };
    let benutzer = benutzername(uid);
    protokoll(&format!(
        "Anfrage: {aktion} für {benutzer} (uid {uid}), Cookie {cookie}"
    ));
    let (window, eingabe, status, legitimieren, abbrechen_knopf) =
        fenster(app, &aktion, &nachricht, &symbol, &benutzer);
    let anfrage = Rc::new(RefCell::new(Anfrage {
        cookie,
        uid,
        benutzer,
        invocation: Some(invocation),
        window: window.clone(),
        eingabe: eingabe.clone(),
        status,
        knopf: legitimieren.clone(),
        session: None,
        versuche: 0,
        wartet_auf_antwort: false,
    }));
    OFFEN.with(|o| *o.borrow_mut() = Some(anfrage.clone()));

    eingabe.connect_activate({
        let polkit = polkit.clone();
        let anfrage = anfrage.clone();
        move |_| antworten(&polkit, &anfrage)
    });
    legitimieren.connect_clicked({
        let polkit = polkit.clone();
        let anfrage = anfrage.clone();
        move |_| antworten(&polkit, &anfrage)
    });
    abbrechen_knopf.connect_clicked({
        let polkit = polkit.clone();
        let anfrage = anfrage.clone();
        move |_| abbrechen(&polkit, &anfrage, "Schaltfläche Abbrechen")
    });
    let tasten = gtk::EventControllerKey::new();
    tasten.connect_key_pressed({
        let polkit = polkit.clone();
        let anfrage = anfrage.clone();
        move |_, key, _, _| {
            if key == gdk::Key::Escape {
                abbrechen(&polkit, &anfrage, "Esc");
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    window.add_controller(tasten);
    window.connect_close_request({
        let polkit = polkit.clone();
        let anfrage = anfrage.clone();
        move |_| {
            if anfrage.borrow().invocation.is_some() {
                abbrechen(&polkit, &anfrage, "Fenster geschlossen");
            }
            glib::Propagation::Proceed
        }
    });
    window.present();
    sitzung_starten(polkit, &anfrage);
}

/// `CancelAuthentication` von polkitd (z. B. Anfrager beendet).
fn abbruch_von_polkit(
    polkit: &Polkit,
    parameter: &glib::Variant,
    invocation: gio::DBusMethodInvocation,
) {
    let cookie = parameter
        .get::<(String,)>()
        .map(|(c,)| c)
        .unwrap_or_default();
    let offen = OFFEN.with(|o| o.borrow().clone());
    match offen {
        Some(anfrage) if anfrage.borrow().cookie == cookie => {
            let benutzer = anfrage.borrow().benutzer.clone();
            protokoll(&format!("polkitd zieht die Anfrage für {benutzer} zurück"));
            abschliessen(
                polkit,
                &anfrage,
                Err((
                    "org.freedesktop.PolicyKit1.Error.Cancelled",
                    "Von polkitd zurückgezogen".to_string(),
                )),
            );
        }
        _ => protokoll(&format!(
            "CancelAuthentication für unbekanntes Cookie {cookie}"
        )),
    }
    invocation.return_value(None);
}

/// Startzeit dieses Prozesses (Feld 22 von /proc/self/stat), wie polkit
/// sie für `unix-process` erwartet.
fn startzeit() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let rest = stat.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// Subjekt für die Registrierung, in dieser Reihenfolge: die logind-Sitzung
/// aus `XDG_SESSION_ID` (in der Querencia-Sitzung vom Login gesetzt), sonst
/// die Sitzung dieses Prozesses laut logind (`GetSessionByPID`), sonst der
/// Prozess selbst (polkitd ordnet ihn zu, wenn er in einer Sitzung läuft).
fn subjekt(bus: &gio::DBusConnection) -> (glib::Variant, String) {
    let sitzung = std::env::var("XDG_SESSION_ID")
        .ok()
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric()))
        .or_else(|| sitzung_laut_logind(bus));
    if let Some(id) = sitzung {
        let mut werte = std::collections::HashMap::<String, glib::Variant>::new();
        werte.insert("session-id".to_string(), id.to_variant());
        return (
            ("unix-session".to_string(), werte).to_variant(),
            format!("unix-session {id}"),
        );
    }
    let mut werte = std::collections::HashMap::<String, glib::Variant>::new();
    werte.insert("pid".to_string(), (std::process::id()).to_variant());
    werte.insert(
        "start-time".to_string(),
        startzeit().unwrap_or(0).to_variant(),
    );
    (
        ("unix-process".to_string(), werte).to_variant(),
        format!("unix-process {}", std::process::id()),
    )
}

/// Sitzungskennung dieses Prozesses laut logind (`/org/freedesktop/login1/session/_310` → `10`).
fn sitzung_laut_logind(bus: &gio::DBusConnection) -> Option<String> {
    let antwort = bus
        .call_sync(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "GetSessionByPID",
            Some(&(std::process::id(),).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            5000,
            None::<&gio::Cancellable>,
        )
        .ok()?;
    let (pfad,) = antwort.get::<(glib::variant::ObjectPath,)>()?;
    let kennung = bus
        .call_sync(
            Some("org.freedesktop.login1"),
            pfad.as_str(),
            "org.freedesktop.DBus.Properties",
            "Get",
            Some(&("org.freedesktop.login1.Session", "Id").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            5000,
            None::<&gio::Cancellable>,
        )
        .ok()?;
    let (wert,) = kennung.get::<(glib::Variant,)>()?;
    wert.get::<String>().filter(|s| !s.is_empty())
}

/// Stellt das Objekt `/org/querencia/PolkitAgent` am System-Bus bereit.
fn objekt_anmelden(
    app: &gtk::Application,
    polkit: Rc<Polkit>,
    bus: &gio::DBusConnection,
) -> Result<(), String> {
    let info =
        gio::DBusNodeInfo::for_xml(SCHNITTSTELLE).map_err(|e| format!("Schnittstelle: {e}"))?;
    let iface = info
        .lookup_interface("org.freedesktop.PolicyKit1.AuthenticationAgent")
        .ok_or("Schnittstelle fehlt")?;
    let app = app.clone();
    bus.register_object(OBJEKT, &iface)
        .method_call(move |_, sender, _, _, methode, parameter, invocation| {
            // Nur polkitd (als root) darf Anfragen stellen; die D-Bus-Richtlinie
            // lässt die Methoden nur für Nachrichten von polkitd zu, trotzdem
            // Absender mitprotokollieren.
            protokoll(&format!("{methode} von {}", sender.unwrap_or("?")));
            match methode {
                "BeginAuthentication" => beginnen(&app, &polkit, &parameter, invocation),
                "CancelAuthentication" => abbruch_von_polkit(&polkit, &parameter, invocation),
                _ => invocation
                    .return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", methode),
            }
        })
        .build()
        .map_err(|e| format!("Objekt {OBJEKT} nicht registrierbar: {e}"))?;
    Ok(())
}

/// Alles für `RegisterAuthenticationAgent`, einmal beim Start bestimmt:
/// jede erneute Registrierung nennt dasselbe Subjekt und dieselbe Locale.
struct Registrierung {
    bus: gio::DBusConnection,
    subjekt: glib::Variant,
    beschreibung: String,
    locale: String,
    /// Eindeutiger Busname des polkitd, bei dem der Agent registriert ist.
    besitzer: RefCell<Option<String>>,
    /// Zählt die Besitzerwechsel; ein Versuch aus einer älteren Runde verfällt.
    runde: Cell<u64>,
}

impl Registrierung {
    fn bestimmen(bus: gio::DBusConnection) -> Registrierung {
        let locale = locale_fuer_polkit(
            std::env::var("LANGUAGE").ok().as_deref(),
            std::env::var("LC_ALL")
                .or_else(|_| std::env::var("LC_MESSAGES"))
                .or_else(|_| std::env::var("LANG"))
                .ok()
                .as_deref(),
        );
        let (subjekt, beschreibung) = subjekt(&bus);
        Registrierung {
            bus,
            subjekt,
            beschreibung,
            locale,
            besitzer: RefCell::new(None),
            runde: Cell::new(0),
        }
    }

    fn parameter(&self) -> glib::Variant {
        glib::Variant::tuple_from_iter([
            self.subjekt.clone(),
            self.locale.to_variant(),
            OBJEKT.to_variant(),
        ])
    }

    fn angaben(&self) -> String {
        format!(
            "Subjekt {}, Locale {}, Objekt {OBJEKT}",
            self.beschreibung, self.locale
        )
    }

    /// Eindeutiger Name des jetzigen Besitzers von org.freedesktop.PolicyKit1.
    fn besitzer_abfragen(&self) -> Option<String> {
        let antwort = self
            .bus
            .call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "GetNameOwner",
                Some(&(POLKIT_NAME,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                2000,
                None::<&gio::Cancellable>,
            )
            .ok()?;
        antwort.get::<(String,)>().map(|(n,)| n)
    }

    /// Erste Registrierung beim Start (polkitd darf dafür starten).
    fn erstmals(&self) -> Result<(), String> {
        let vorher = self.besitzer_abfragen();
        self.bus
            .call_sync(
                Some(POLKIT_NAME),
                POLKIT_PFAD,
                POLKIT_SCHNITTSTELLE,
                "RegisterAuthenticationAgent",
                Some(&self.parameter()),
                None,
                gio::DBusCallFlags::NONE,
                10_000,
                None::<&gio::Cancellable>,
            )
            .map_err(|e| format!("RegisterAuthenticationAgent: {e}"))?;
        // Lief polkitd noch nicht, hat erst dieser Aufruf ihn gestartet.
        let besitzer = vorher.or_else(|| self.besitzer_abfragen());
        protokoll(&format!(
            "registriert bei polkitd (PID {}, {}, polkitd {})",
            std::process::id(),
            self.angaben(),
            besitzer.as_deref().unwrap_or("?")
        ));
        *self.besitzer.borrow_mut() = besitzer;
        Ok(())
    }
}

/// Was ein `NameOwnerChanged` für org.freedesktop.PolicyKit1 bedeutet.
#[derive(Debug, PartialEq, Eq)]
pub enum Wechsel {
    /// Der Besitzer ist der, bei dem der Agent registriert ist.
    Bekannt,
    /// polkitd ist weg, noch kein neuer.
    Beendet,
    /// Ein neuer polkitd; `vorgaenger`: der alte ist dabei verschwunden.
    Neu { vorgaenger: bool },
}

pub fn wechsel_bewerten(registriert_bei: Option<&str>, alt: &str, neu: &str) -> Wechsel {
    if neu.is_empty() {
        Wechsel::Beendet
    } else if registriert_bei == Some(neu) {
        Wechsel::Bekannt
    } else {
        Wechsel::Neu {
            vorgaenger: !alt.is_empty(),
        }
    }
}

/// Wartezeit nach dem fehlgeschlagenen Versuch `versuch` (ab 1); `None`:
/// keine weitere Wiederholung.
pub fn wartezeit(versuch: usize) -> Option<u64> {
    versuch
        .checked_sub(1)
        .and_then(|i| WIEDERHOLUNG_MS.get(i))
        .copied()
}

/// polkitd lehnt eine zweite Registrierung für dasselbe Subjekt ab
/// („An authentication agent already exists for the given subject“).
pub fn schon_registriert(fehler: &str) -> bool {
    fehler.contains("already exists")
}

/// Eine offene Anfrage gehört zum alten polkitd; der neue kennt ihr Cookie
/// nicht.
fn offene_anfrage_verwerfen(polkit: &Polkit) {
    if let Some(anfrage) = OFFEN.with(|o| o.borrow().clone()) {
        abbrechen(polkit, &anfrage, "polkitd beendet");
    }
}

fn besitzerwechsel(reg: &Rc<Registrierung>, polkit: &Polkit, alt: &str, neu: &str) {
    let bisher = reg.besitzer.borrow().clone();
    match wechsel_bewerten(bisher.as_deref(), alt, neu) {
        Wechsel::Bekannt => {}
        Wechsel::Beendet => {
            reg.runde.set(reg.runde.get() + 1);
            *reg.besitzer.borrow_mut() = None;
            protokoll(&format!(
                "polkitd beendet (Besitzer {}), warte auf den neuen",
                if alt.is_empty() { "?" } else { alt }
            ));
            offene_anfrage_verwerfen(polkit);
        }
        Wechsel::Neu { vorgaenger } => {
            reg.runde.set(reg.runde.get() + 1);
            *reg.besitzer.borrow_mut() = None;
            protokoll(&format!(
                "neuer polkitd {neu} (vorher {}), registriere erneut",
                if alt.is_empty() { "keiner" } else { alt }
            ));
            if vorgaenger {
                offene_anfrage_verwerfen(polkit);
            }
            erneut_registrieren(reg, reg.runde.get(), 1, neu.to_string());
        }
    }
}

/// Ein Versuch beim neuen polkitd `ziel`; nur solange die Runde gilt.
fn erneut_registrieren(reg: &Rc<Registrierung>, runde: u64, versuch: usize, ziel: String) {
    if reg.runde.get() != runde {
        return;
    }
    let reg2 = reg.clone();
    // NO_AUTO_START: der neue polkitd hat den Namen schon; ist er inzwischen
    // wieder weg, zählt ohnehin die nächste Runde.
    reg.bus.call(
        Some(POLKIT_NAME),
        POLKIT_PFAD,
        POLKIT_SCHNITTSTELLE,
        "RegisterAuthenticationAgent",
        Some(&reg.parameter()),
        None,
        gio::DBusCallFlags::NO_AUTO_START,
        NEU_ZEITGRENZE_MS,
        None::<&gio::Cancellable>,
        move |ergebnis| {
            if reg2.runde.get() != runde {
                return;
            }
            match ergebnis {
                Ok(_) => {
                    protokoll(&format!(
                        "polkitd neu gestartet, erneut registriert (polkitd {ziel}, Versuch {versuch}, {})",
                        reg2.angaben()
                    ));
                    *reg2.besitzer.borrow_mut() = Some(ziel);
                }
                Err(e) if schon_registriert(&e.to_string()) => {
                    protokoll(&format!(
                        "polkitd {ziel} kennt schon einen Agenten für {} ({e}); keine weitere Wiederholung",
                        reg2.beschreibung
                    ));
                    *reg2.besitzer.borrow_mut() = Some(ziel);
                }
                Err(e) => match wartezeit(versuch) {
                    Some(ms) => {
                        protokoll(&format!(
                            "polkitd {ziel} antwortet noch nicht (Versuch {versuch}: {e}), nächster Versuch in {ms} ms"
                        ));
                        let reg3 = reg2.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(ms),
                            move || erneut_registrieren(&reg3, runde, versuch + 1, ziel),
                        );
                    }
                    None => protokoll(&format!(
                        "erneute Registrierung bei polkitd {ziel} nach {versuch} Versuchen aufgegeben ({e}); erst der nächste Besitzerwechsel versucht es wieder"
                    )),
                },
            }
        },
    );
}

/// Beobachtet den Besitzer von org.freedesktop.PolicyKit1. Das Abo gilt,
/// solange der Rückgabewert lebt.
fn wache_einrichten(reg: &Rc<Registrierung>, polkit: &Rc<Polkit>) -> gio::SignalSubscription {
    let reg = reg.clone();
    let polkit = polkit.clone();
    let bus = reg.bus.clone();
    bus.subscribe_to_signal(
        Some("org.freedesktop.DBus"),
        Some("org.freedesktop.DBus"),
        Some("NameOwnerChanged"),
        Some("/org/freedesktop/DBus"),
        Some(POLKIT_NAME),
        gio::DBusSignalFlags::NONE,
        move |signal| {
            if let Some((name, alt, neu)) = signal.parameters.get::<(String, String, String)>()
                && name == POLKIT_NAME
            {
                besitzerwechsel(&reg, &polkit, &alt, &neu);
            }
        },
    )
}

/// Objekt anmelden, Wache einrichten, dann registrieren. Die Wache steht vor
/// der ersten Registrierung, damit kein Wechsel dazwischen verloren geht.
fn registrieren(
    app: &gtk::Application,
    polkit: Rc<Polkit>,
) -> Result<(Rc<Registrierung>, gio::SignalSubscription), String> {
    let bus = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>)
        .map_err(|e| format!("System-D-Bus nicht erreichbar: {e}"))?;
    objekt_anmelden(app, polkit.clone(), &bus)?;
    let reg = Rc::new(Registrierung::bestimmen(bus));
    let wache = wache_einrichten(&reg, &polkit);
    reg.erstmals()?;
    Ok((reg, wache))
}

/// Locale für die Meldungen von polkitd: wie in der Querencia-Sitzung
/// (`LANG=en_US.UTF-8`, `LANGUAGE=de`) zählt zuerst `LANGUAGE`; ein nackter
/// Sprachcode wird zu `de_DE.UTF-8`, sonst gilt `LC_ALL`/`LC_MESSAGES`/`LANG`.
pub fn locale_fuer_polkit(language: Option<&str>, lang: Option<&str>) -> String {
    if let Some(erste) = language
        .and_then(|l| l.split(':').next())
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && l.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        })
    {
        if erste.contains('_') {
            return if erste.contains('.') {
                erste.to_string()
            } else {
                format!("{erste}.UTF-8")
            };
        }
        return format!("{erste}_{}.UTF-8", erste.to_ascii_uppercase());
    }
    lang.filter(|l| !l.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "de_DE.UTF-8".to_string())
}

fn css_laden() {
    // Dunkelmodus auch, wenn die Sitzung schon dunkel startet (QD-109).
    farbschema::startfehler_beheben("querencia-polkit-agent");
    let css = gtk::CssProvider::new();
    css.load_from_string("window.qs-dialog button { padding: 4px 10px; min-width: 72px; } window.qs-dialog entry { min-width: 260px; }");
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn main() {
    i18n::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("querencia-polkit-agent {VERSION}");
            return;
        }
        Some("--hilfe") | Some("--help") | Some("-h") => {
            print!(
                "Aufruf: querencia-polkit-agent\n\nRegistriert sich als polkit-Legitimierungsagent der Sitzung und zeigt\nbei Bedarf den Passwortdialog. Läuft bis zum Sitzungsende (Autostart).\nStartet polkitd neu, registriert er sich beim neuen polkitd erneut.\n"
            );
            return;
        }
        Some(andere) => {
            eprintln!("querencia-polkit-agent: unbekannte Option {andere}");
            std::process::exit(2);
        }
        None => {}
    }
    let polkit = match Polkit::laden() {
        Ok(p) => p,
        Err(e) => {
            protokoll(&format!("libpolkit nicht ladbar: {e}"));
            std::process::exit(1);
        }
    };
    let app = gtk::Application::builder().application_id(APP_ID).build();
    // Kein Fenster beim Start: die Anwendung lebt, bis die Sitzung endet.
    let haltung: Rc<RefCell<Option<gio::ApplicationHoldGuard>>> = Rc::new(RefCell::new(None));
    type Halt = (Rc<Registrierung>, gio::SignalSubscription);
    let registrierung: Rc<RefCell<Option<Halt>>> = Rc::new(RefCell::new(None));
    app.connect_startup({
        let polkit = polkit.clone();
        let haltung = haltung.clone();
        let registrierung = registrierung.clone();
        move |app| {
            css_laden();
            match registrieren(app, polkit.clone()) {
                Ok(r) => {
                    *registrierung.borrow_mut() = Some(r);
                    *haltung.borrow_mut() = Some(app.hold());
                }
                Err(e) => {
                    protokoll(&e);
                    std::process::exit(1);
                }
            }
        }
    });
    app.connect_activate(|_| {});
    let code = app.run_with_args::<String>(&[]);
    std::process::exit(code.into());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eigener_benutzer_zuerst_sonst_erster_unix_benutzer() {
        let liste = vec![
            ("unix-user".to_string(), 0),
            ("unix-user".to_string(), 1001),
        ];
        assert_eq!(identitaet_waehlen(&liste, 1001), Some(1001));
        assert_eq!(identitaet_waehlen(&liste, 1002), Some(0));
        let gruppe = vec![("unix-group".to_string(), 27)];
        assert_eq!(identitaet_waehlen(&gruppe, 1001), None);
        assert_eq!(identitaet_waehlen(&[], 1001), None);
    }

    #[test]
    fn startzeit_lesbar() {
        assert!(startzeit().is_some_and(|t| t > 0));
    }

    /// Liest nur: Sitzung dieses Prozesses laut logind oder Prozess-Subjekt.
    #[test]
    fn subjekt_ist_sitzung_oder_prozess() {
        let Ok(bus) = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>) else {
            return;
        };
        let (s, beschreibung) = subjekt(&bus);
        let (art, werte) = s
            .get::<(String, std::collections::HashMap<String, glib::Variant>)>()
            .unwrap();
        match art.as_str() {
            "unix-session" => {
                assert!(
                    werte["session-id"]
                        .get::<String>()
                        .is_some_and(|id| !id.is_empty())
                );
                assert!(beschreibung.starts_with("unix-session "));
            }
            "unix-process" => {
                assert_eq!(werte["pid"].get::<u32>(), Some(std::process::id()));
                assert!(werte["start-time"].get::<u64>().is_some());
            }
            andere => panic!("{andere}"),
        }
    }

    #[test]
    fn locale_aus_language_zuerst() {
        assert_eq!(
            locale_fuer_polkit(Some("de"), Some("en_US.UTF-8")),
            "de_DE.UTF-8"
        );
        assert_eq!(
            locale_fuer_polkit(Some("de:en"), Some("en_US.UTF-8")),
            "de_DE.UTF-8"
        );
        assert_eq!(locale_fuer_polkit(Some("de_AT"), None), "de_AT.UTF-8");
        assert_eq!(locale_fuer_polkit(Some("de_DE.UTF-8"), None), "de_DE.UTF-8");
        assert_eq!(locale_fuer_polkit(None, Some("en_US.UTF-8")), "en_US.UTF-8");
        assert_eq!(locale_fuer_polkit(Some(""), None), "de_DE.UTF-8");
        assert_eq!(locale_fuer_polkit(Some("../x"), Some("C")), "C");
    }

    #[test]
    fn besitzerwechsel_bewerten() {
        // Start: registriert bei :1.5
        assert_eq!(wechsel_bewerten(Some(":1.5"), "", ":1.5"), Wechsel::Bekannt);
        assert_eq!(wechsel_bewerten(Some(":1.5"), ":1.5", ""), Wechsel::Beendet);
        assert_eq!(wechsel_bewerten(None, ":1.5", ""), Wechsel::Beendet);
        assert_eq!(
            wechsel_bewerten(None, "", ":1.9"),
            Wechsel::Neu { vorgaenger: false }
        );
        // derselbe Prozess gibt den Namen ab und nimmt ihn wieder
        assert_eq!(
            wechsel_bewerten(None, "", ":1.5"),
            Wechsel::Neu { vorgaenger: false }
        );
        // Übernahme ohne Lücke
        assert_eq!(
            wechsel_bewerten(Some(":1.5"), ":1.5", ":1.9"),
            Wechsel::Neu { vorgaenger: true }
        );
    }

    #[test]
    fn wartezeit_waechst_und_endet() {
        assert_eq!(wartezeit(0), None);
        assert_eq!(wartezeit(1), Some(500));
        assert_eq!(wartezeit(2), Some(1000));
        assert_eq!(wartezeit(6), Some(15000));
        assert_eq!(wartezeit(11), Some(30000));
        assert_eq!(wartezeit(12), None);
        let summe: u64 = (1..=11).filter_map(wartezeit).sum();
        assert_eq!(summe, 180_500);
    }

    #[test]
    fn zweite_registrierung_erkannt() {
        assert!(schon_registriert(
            "GDBus.Error:org.freedesktop.PolicyKit1.Error.Failed: An authentication agent already exists for the given subject"
        ));
        assert!(!schon_registriert(
            "GDBus.Error:org.freedesktop.DBus.Error.NoReply: Timeout was reached"
        ));
    }

    #[test]
    fn libpolkit_ladbar() {
        assert!(Polkit::laden().is_ok());
    }

    #[test]
    fn schnittstelle_gueltig() {
        let info = gio::DBusNodeInfo::for_xml(SCHNITTSTELLE).unwrap();
        assert!(
            info.lookup_interface("org.freedesktop.PolicyKit1.AuthenticationAgent")
                .is_some()
        );
    }
}
