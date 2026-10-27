//! Sperrbildschirm über das Wayland-Protokoll ext-session-lock-v1.
//!
//! Spricht Wayland direkt (wayland-client), ohne GTK: die C-Bibliothek
//! gtk4-session-lock, die das Crate `gtk4-session-lock` braucht, ist auf der
//! VM nicht installiert, und ein Sperrbildschirm braucht keine Toplevel-
//! Fenster. Gezeichnet wird mit Cairo und Pango in Shared-Memory-Puffer
//! (`wl_shm`), eine Sperrfläche je Ausgang (auch für Ausgänge, die während
//! der Sperre hinzukommen). Tastatur über `wl_keyboard` und xkbcommon.
//!
//! Optik: Anthrazit, Querencia-Q, Uhr, Datum, Benutzername, Passwortfeld,
//! Meldung (Akzent #F07746). Entsperren über PAM (`pam.rs`) in einem
//! eigenen Thread, damit die Uhr weiterläuft; Enter prüft, Esc leert,
//! Rücktaste löscht, Strg+U leert.
//!
//! Prüfmodus: mit `QS_TROCKENLAUF=1`, `QS_TESTPASSWORT` und einem
//! `XDG_RUNTIME_DIR` außerhalb von `/run/user/` (also nur in einer
//! isolierten Testsitzung) entsperrt das Testpasswort ohne PAM.

use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

use crate::i18n::{gettext, gettext_f};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_shm, wl_shm_pool,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
};
use xkbcommon::xkb;

use crate::aktion::protokoll;
use crate::pam;
use crate::zeit;

const HINTERGRUND: (f64, f64, f64) = (0.180, 0.204, 0.212); // #2e3436 Anthrazit
const FELD: (f64, f64, f64) = (0.118, 0.133, 0.141);
const RAHMEN: (f64, f64, f64) = (0.541, 0.561, 0.576);
const TEXT: (f64, f64, f64) = (0.933, 0.933, 0.925);
const GEDIMMT: (f64, f64, f64) = (0.667, 0.686, 0.698);
const AKZENT: (f64, f64, f64) = (0.941, 0.467, 0.275); // #F07746
const LOGO_PNG: &[u8] = include_bytes!("../data/logo/querencia-logo-128.png");
const MAX_PASSWORT: usize = 256;

/// Testpasswort nur im Trockenlauf und nur außerhalb einer echten Sitzung
/// (`XDG_RUNTIME_DIR` nicht unter `/run/user/`).
pub fn testpasswort(
    trockenlauf: bool,
    runtime_dir: Option<&str>,
    testpasswort: Option<&str>,
) -> Option<String> {
    let dir = runtime_dir?;
    if !trockenlauf || dir.starts_with("/run/user/") || dir.is_empty() {
        return None;
    }
    testpasswort.filter(|p| !p.is_empty()).map(str::to_string)
}

/// Was eine Taste im Passwortfeld bewirkt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taste {
    Zeichen(String),
    Loeschen,
    Leeren,
    Pruefen,
    Nichts,
}

/// Deutet Keysym, Strg-Zustand und UTF-8-Text einer gedrückten Taste.
pub fn taste(keysym: u32, strg: bool, text: &str) -> Taste {
    use xkb::keysyms as k;
    match keysym {
        k::KEY_Return | k::KEY_KP_Enter => Taste::Pruefen,
        k::KEY_BackSpace => Taste::Loeschen,
        k::KEY_Escape => Taste::Leeren,
        k::KEY_u | k::KEY_U if strg => Taste::Leeren,
        _ if strg => Taste::Nichts,
        _ => {
            if !text.is_empty() && text.chars().all(|c| !c.is_control()) {
                Taste::Zeichen(text.to_string())
            } else {
                Taste::Nichts
            }
        }
    }
}

struct Puffer {
    buffer: wl_buffer::WlBuffer,
    pool: wl_shm_pool::WlShmPool,
    speicher: *mut u8,
    laenge: usize,
    breite: i32,
    hoehe: i32,
    stride: i32,
    belegt: Arc<AtomicBool>,
}

impl Puffer {
    fn neu(
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<Sperre>,
        breite: i32,
        hoehe: i32,
    ) -> Result<Puffer, String> {
        let stride = cairo::Format::ARgb32
            .stride_for_width(breite as u32)
            .map_err(|e| format!("Cairo-Stride: {e:?}"))?;
        let laenge = (stride * hoehe) as usize;
        // SAFETY: memfd_create/ftruncate/mmap mit geprüften Rückgabewerten;
        // der Speicher wird in `Drop` wieder freigegeben.
        let (fd, speicher) = unsafe {
            let fd = libc::memfd_create(c"querencia-sitzung-sperre".as_ptr(), libc::MFD_CLOEXEC);
            if fd < 0 {
                return Err("memfd_create fehlgeschlagen".to_string());
            }
            let fd = OwnedFd::from_raw_fd(fd);
            if libc::ftruncate(fd.as_raw_fd(), laenge as libc::off_t) < 0 {
                return Err("ftruncate fehlgeschlagen".to_string());
            }
            let p = libc::mmap(
                std::ptr::null_mut(),
                laenge,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            );
            if p == libc::MAP_FAILED {
                return Err("mmap fehlgeschlagen".to_string());
            }
            (fd, p as *mut u8)
        };
        let pool = shm.create_pool(fd.as_fd(), laenge as i32, qh, ());
        let belegt = Arc::new(AtomicBool::new(false));
        let buffer = pool.create_buffer(
            0,
            breite,
            hoehe,
            stride,
            wl_shm::Format::Argb8888,
            qh,
            belegt.clone(),
        );
        Ok(Puffer {
            buffer,
            pool,
            speicher,
            laenge,
            breite,
            hoehe,
            stride,
            belegt,
        })
    }

    fn passt(&self, breite: i32, hoehe: i32) -> bool {
        self.breite == breite && self.hoehe == hoehe
    }
}

impl Drop for Puffer {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
        // SAFETY: speicher/laenge stammen aus dem mmap in `neu`.
        unsafe {
            libc::munmap(self.speicher as *mut libc::c_void, self.laenge);
        }
    }
}

struct Ausgang {
    global: u32,
    output: wl_output::WlOutput,
    name: String,
    surface: Option<wl_surface::WlSurface>,
    lock_surface: Option<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1>,
    breite: i32,
    hoehe: i32,
    puffer: Vec<Puffer>,
}

/// Zustand des Sperrbildschirms.
pub struct Sperre {
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    lock: Option<ext_session_lock_v1::ExtSessionLockV1>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    ausgaenge: Vec<Ausgang>,
    xkb_context: xkb::Context,
    xkb_state: Option<xkb::State>,
    logo: Option<cairo::ImageSurface>,
    benutzer: String,
    dienst: String,
    testpasswort: Option<String>,
    passwort: String,
    meldung: Option<(String, bool)>, // (Text, ist Fehler)
    pruefung: Option<mpsc::Receiver<pam::Ergebnis>>,
    weck_schreiber: Option<OwnedFd>,
    gesperrt: bool,
    fertig: Option<i32>,
    neu_zeichnen: bool,
    uhr: (String, String),
}

impl Sperre {
    fn ausgang_mut(&mut self, global: u32) -> Option<&mut Ausgang> {
        self.ausgaenge.iter_mut().find(|a| a.global == global)
    }

    /// Legt für einen Ausgang die Sperrfläche an (sobald der Lock da ist).
    fn sperrflaeche(&mut self, global: u32, qh: &QueueHandle<Sperre>) {
        let Some(lock) = self.lock.clone() else {
            return;
        };
        let compositor = self.compositor.clone();
        let Some(a) = self.ausgang_mut(global) else {
            return;
        };
        if a.lock_surface.is_some() {
            return;
        }
        let surface = compositor.create_surface(qh, ());
        let lock_surface = lock.get_lock_surface(&surface, &a.output, qh, global);
        a.surface = Some(surface);
        a.lock_surface = Some(lock_surface);
    }

    fn zeichnen(&mut self, global: u32, qh: &QueueHandle<Sperre>) -> Result<(), String> {
        let uhr = self.uhr.clone();
        let passwort_laenge = self.passwort.chars().count();
        let meldung = self.meldung.clone();
        let prueft = self.pruefung.is_some();
        let feststell = self
            .xkb_state
            .as_ref()
            .is_some_and(|s| s.mod_name_is_active(xkb::MOD_NAME_CAPS, xkb::STATE_MODS_EFFECTIVE));
        let benutzer = self.benutzer.clone();
        let logo = self.logo.clone();
        let shm = self.shm.clone();
        let Some(a) = self.ausgang_mut(global) else {
            return Ok(());
        };
        let (Some(surface), Some(_)) = (&a.surface, &a.lock_surface) else {
            return Ok(());
        };
        if a.breite <= 0 || a.hoehe <= 0 {
            return Ok(());
        }
        let (breite, hoehe) = (a.breite, a.hoehe);
        a.puffer
            .retain(|p| p.passt(breite, hoehe) || p.belegt.load(Ordering::Acquire));
        let index = match a
            .puffer
            .iter()
            .position(|p| p.passt(breite, hoehe) && !p.belegt.load(Ordering::Acquire))
        {
            Some(i) => i,
            None => {
                a.puffer.push(Puffer::neu(&shm, qh, breite, hoehe)?);
                a.puffer.len() - 1
            }
        };
        let puffer = &mut a.puffer[index];
        let mut bild = cairo::ImageSurface::create(cairo::Format::ARgb32, breite, hoehe)
            .map_err(|e| format!("Cairo-Fläche: {e}"))?;
        {
            let cr = cairo::Context::new(&bild).map_err(|e| format!("Cairo: {e}"))?;
            malen(
                &cr,
                breite as f64,
                hoehe as f64,
                &Inhalt {
                    uhr: &uhr,
                    benutzer: &benutzer,
                    passwort_laenge,
                    meldung: meldung.as_ref(),
                    prueft,
                    feststell,
                    logo: logo.as_ref(),
                },
            );
        }
        bild.flush();
        let stride = bild.stride();
        let daten = bild.data().map_err(|e| format!("Cairo-Daten: {e}"))?;
        // SAFETY: Ziel ist der gemappte Puffer dieser Größe; Quelle und Ziel
        // haben denselben Stride (cairo::Format::stride_for_width).
        unsafe {
            let ziel = std::slice::from_raw_parts_mut(puffer.speicher, puffer.laenge);
            let n = (stride.min(puffer.stride) * hoehe) as usize;
            ziel[..n].copy_from_slice(&daten[..n]);
        }
        puffer.belegt.store(true, Ordering::Release);
        surface.attach(Some(&puffer.buffer), 0, 0);
        surface.damage_buffer(0, 0, breite, hoehe);
        surface.commit();
        Ok(())
    }

    fn alle_zeichnen(&mut self, qh: &QueueHandle<Sperre>) {
        let globals: Vec<u32> = self.ausgaenge.iter().map(|a| a.global).collect();
        for g in globals {
            if let Err(e) = self.zeichnen(g, qh) {
                protokoll(&format!("Zeichnen auf Ausgang {g}: {e}"));
            }
        }
        self.neu_zeichnen = false;
    }

    fn weck_fd(&mut self) -> Option<OwnedFd> {
        self.weck_schreiber
            .as_ref()
            .and_then(|fd| fd.try_clone().ok())
    }

    /// Enter: Passwort prüfen (PAM im Thread oder Testpasswort).
    fn pruefen(&mut self) {
        if self.pruefung.is_some() {
            return;
        }
        let passwort = std::mem::take(&mut self.passwort);
        if passwort.is_empty() {
            self.meldung = Some((gettext("Please enter your password"), false));
            self.neu_zeichnen = true;
            return;
        }
        if let Some(test) = &self.testpasswort {
            protokoll("TROCKENLAUF: Testpasswort statt PAM geprüft");
            if *test == passwort {
                self.entsperren();
            } else {
                self.meldung = Some((gettext("Wrong password"), true));
            }
            self.neu_zeichnen = true;
            return;
        }
        let (tx, rx) = mpsc::channel();
        let dienst = self.dienst.clone();
        let benutzer = self.benutzer.clone();
        let wecker = self.weck_fd();
        std::thread::spawn(move || {
            let ergebnis = pam::pruefen(&dienst, &benutzer, &passwort);
            let _ = tx.send(ergebnis);
            if let Some(fd) = wecker {
                // SAFETY: gültiger Dateideskriptor der Weckpipe.
                unsafe {
                    libc::write(fd.as_raw_fd(), b"w".as_ptr() as *const libc::c_void, 1);
                }
            }
        });
        self.pruefung = Some(rx);
        self.meldung = Some((gettext("Checking password…"), false));
        self.neu_zeichnen = true;
    }

    /// Holt ein fertiges PAM-Ergebnis ab.
    fn pruefung_abholen(&mut self) {
        let Some(rx) = &self.pruefung else { return };
        let Ok(ergebnis) = rx.try_recv() else { return };
        self.pruefung = None;
        match ergebnis {
            pam::Ergebnis::Richtig => {
                protokoll("Passwort richtig, Sitzung wird entsperrt");
                self.entsperren();
            }
            pam::Ergebnis::Falsch(m) => {
                protokoll(&format!("Passwort abgelehnt: {m}"));
                self.meldung = Some((m, true));
            }
            pam::Ergebnis::Fehler(m) => {
                protokoll(&format!("PAM-Fehler: {m}"));
                self.meldung = Some((
                    gettext_f("Cannot check the password: {error}", &[("error", &m)]),
                    true,
                ));
            }
        }
        self.neu_zeichnen = true;
    }

    fn entsperren(&mut self) {
        if let Some(lock) = self.lock.take() {
            lock.unlock_and_destroy();
        }
        for a in &mut self.ausgaenge {
            if let Some(ls) = a.lock_surface.take() {
                ls.destroy();
            }
            if let Some(s) = a.surface.take() {
                s.destroy();
            }
        }
        self.fertig = Some(0);
    }

    fn taste_gedrueckt(&mut self, keysym: u32, strg: bool, text: &str) {
        match taste(keysym, strg, text) {
            Taste::Zeichen(z) => {
                if self.pruefung.is_none() && self.passwort.chars().count() < MAX_PASSWORT {
                    self.passwort.push_str(&z);
                    self.meldung = None;
                }
            }
            Taste::Loeschen => {
                self.passwort.pop();
            }
            Taste::Leeren => {
                self.passwort.clear();
                self.meldung = None;
            }
            Taste::Pruefen => self.pruefen(),
            Taste::Nichts => return,
        }
        self.neu_zeichnen = true;
    }
}

struct Inhalt<'a> {
    uhr: &'a (String, String),
    benutzer: &'a str,
    passwort_laenge: usize,
    meldung: Option<&'a (String, bool)>,
    prueft: bool,
    feststell: bool,
    logo: Option<&'a cairo::ImageSurface>,
}

fn farbe(cr: &cairo::Context, (r, g, b): (f64, f64, f64)) {
    cr.set_source_rgb(r, g, b);
}

/// Pango-Layout mit Graustufen-Kantenglättung (kein Subpixel-Farbsaum auf
/// dem Anthrazit-Hintergrund).
fn layout(cr: &cairo::Context, schrift: &str) -> pango::Layout {
    let layout = pangocairo::functions::create_layout(cr);
    let mut optionen = cairo::FontOptions::new().ok();
    if let Some(o) = &mut optionen {
        o.set_antialias(cairo::Antialias::Gray);
        o.set_hint_style(cairo::HintStyle::Slight);
        pangocairo::functions::context_set_font_options(&layout.context(), Some(o));
    }
    layout.set_font_description(Some(&pango::FontDescription::from_string(schrift)));
    layout
}

fn text_zentriert(cr: &cairo::Context, x: f64, y: f64, schrift: &str, text: &str) -> f64 {
    let layout = layout(cr, schrift);
    layout.set_text(text);
    let (b, h) = layout.pixel_size();
    cr.move_to(x - b as f64 / 2.0, y);
    pangocairo::functions::show_layout(cr, &layout);
    h as f64
}

fn rundeck(cr: &cairo::Context, x: f64, y: f64, b: f64, h: f64, r: f64) {
    use std::f64::consts::PI;
    cr.new_sub_path();
    cr.arc(x + b - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + b - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, 3.0 * PI / 2.0);
    cr.close_path();
}

/// Zeichnet den Sperrbildschirm für einen Ausgang der Größe `b`×`h`.
fn malen(cr: &cairo::Context, b: f64, h: f64, inhalt: &Inhalt) {
    farbe(cr, HINTERGRUND);
    let _ = cr.paint();
    let klein = h < 700.0;
    let logo_groesse = if klein { 80.0 } else { 112.0 };
    let uhr_schrift = if klein {
        "DejaVu Sans 56"
    } else {
        "DejaVu Sans 72"
    };
    let feld_b = 320.0_f64.min(b - 40.0);
    let feld_h = 44.0;
    let gesamt = logo_groesse + 20.0 + 100.0 + 28.0 + 36.0 + 24.0 + 10.0 + feld_h + 14.0 + 24.0;
    let mitte = b / 2.0;
    let mut y = ((h - gesamt) / 2.0).max(16.0);

    if let Some(logo) = inhalt.logo {
        let (lb, lh) = (logo.width() as f64, logo.height() as f64);
        let s = logo_groesse / lb.max(lh);
        let _ = cr.save();
        cr.translate(mitte - lb * s / 2.0, y);
        cr.scale(s, s);
        let _ = cr.set_source_surface(logo, 0.0, 0.0);
        let _ = cr.paint();
        let _ = cr.restore();
    }
    y += logo_groesse + 20.0;

    farbe(cr, TEXT);
    let uh = text_zentriert(cr, mitte, y, uhr_schrift, &inhalt.uhr.0);
    y += uh + 4.0;
    farbe(cr, GEDIMMT);
    let dh = text_zentriert(cr, mitte, y, "DejaVu Sans 15", &inhalt.uhr.1);
    y += dh + 36.0;

    farbe(cr, TEXT);
    let bh = text_zentriert(cr, mitte, y, "DejaVu Sans Bold 13", inhalt.benutzer);
    y += bh + 10.0;

    // Passwortfeld
    let fx = mitte - feld_b / 2.0;
    rundeck(cr, fx, y, feld_b, feld_h, 6.0);
    farbe(cr, FELD);
    let _ = cr.fill_preserve();
    if inhalt.passwort_laenge > 0 || inhalt.prueft {
        farbe(cr, AKZENT);
    } else {
        farbe(cr, RAHMEN);
    }
    cr.set_line_width(1.5);
    let _ = cr.stroke();
    if inhalt.passwort_laenge == 0 {
        farbe(cr, GEDIMMT);
        let layout = layout(cr, "DejaVu Sans 12");
        let platzhalter = if inhalt.prueft {
            "…".to_string()
        } else {
            // TRANSLATORS: Platzhalter im leeren Passwortfeld des Sperrbildschirms
            gettext("Password")
        };
        layout.set_text(&platzhalter);
        let (_, th) = layout.pixel_size();
        cr.move_to(fx + 12.0, y + (feld_h - th as f64) / 2.0);
        pangocairo::functions::show_layout(cr, &layout);
    } else {
        farbe(cr, TEXT);
        let punkte = inhalt.passwort_laenge.min(40);
        let abstand = 11.0;
        let breite = punkte as f64 * abstand;
        let start = (mitte - breite / 2.0).max(fx + 14.0) + abstand / 2.0;
        for i in 0..punkte {
            cr.arc(
                start + i as f64 * abstand,
                y + feld_h / 2.0,
                3.5,
                0.0,
                std::f64::consts::TAU,
            );
            let _ = cr.fill();
        }
    }
    y += feld_h + 14.0;

    let feststell = gettext("Caps Lock is on");
    let hinweis = gettext("Enter your password and press Enter");
    let (text, fehler) = match inhalt.meldung {
        Some((m, f)) => (m.as_str(), *f),
        None if inhalt.feststell => (feststell.as_str(), true),
        None => (hinweis.as_str(), false),
    };
    farbe(cr, if fehler { AKZENT } else { GEDIMMT });
    let mh = text_zentriert(
        cr,
        mitte,
        y,
        if fehler {
            "DejaVu Sans Bold 12"
        } else {
            "DejaVu Sans 12"
        },
        text,
    );
    if inhalt.feststell && inhalt.meldung.is_some() {
        farbe(cr, AKZENT);
        text_zentriert(cr, mitte, y + mh + 4.0, "DejaVu Sans 11", &feststell);
    }

    farbe(cr, GEDIMMT);
    text_zentriert(cr, mitte, h - 36.0, "DejaVu Sans 10", "Querencia Desktop");
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Sperre {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == "wl_output" => {
                let output =
                    registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, name);
                state.ausgaenge.push(Ausgang {
                    global: name,
                    output,
                    name: String::new(),
                    surface: None,
                    lock_surface: None,
                    breite: 0,
                    hoehe: 0,
                    puffer: Vec::new(),
                });
                protokoll(&format!("Ausgang {name} hinzugekommen"));
                state.sperrflaeche(name, qh);
            }
            wl_registry::Event::GlobalRemove { name } => {
                if let Some(i) = state.ausgaenge.iter().position(|a| a.global == name) {
                    let mut a = state.ausgaenge.remove(i);
                    protokoll(&format!("Ausgang {} ({name}) entfernt", a.name));
                    if let Some(ls) = a.lock_surface.take() {
                        ls.destroy();
                    }
                    if let Some(s) = a.surface.take() {
                        s.destroy();
                    }
                    a.output.release();
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for Sperre {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event
            && let Some(a) = state.ausgang_mut(*global)
        {
            a.name = name;
        }
    }
}

impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for Sperre {
    fn event(
        state: &mut Self,
        _: &ext_session_lock_v1::ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => {
                state.gesperrt = true;
                protokoll("Sitzung gesperrt (ext-session-lock locked)");
            }
            ext_session_lock_v1::Event::Finished => {
                if state.gesperrt {
                    protokoll("Compositor hat die Sperre beendet (finished)");
                    state.fertig = Some(0);
                } else {
                    protokoll("Sperre abgelehnt (finished vor locked): Sitzung schon gesperrt?");
                    state.fertig = Some(2);
                }
                if let Some(lock) = state.lock.take() {
                    lock.destroy();
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1, u32> for Sperre {
    fn event(
        state: &mut Self,
        surface: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        global: &u32,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            surface.ack_configure(serial);
            let name = if let Some(a) = state.ausgang_mut(*global) {
                a.breite = width as i32;
                a.hoehe = height as i32;
                a.name.clone()
            } else {
                return;
            };
            protokoll(&format!("Sperrfläche auf {name}: {width}×{height}"));
            if let Err(e) = state.zeichnen(*global, qh) {
                protokoll(&format!("Zeichnen auf {name}: {e}"));
            }
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, Arc<AtomicBool>> for Sperre {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        belegt: &Arc<AtomicBool>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            belegt.store(false, Ordering::Release);
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Sperre {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            let hat_tastatur = caps.contains(wl_seat::Capability::Keyboard);
            if hat_tastatur && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            } else if !hat_tastatur && let Some(k) = state.keyboard.take() {
                k.release();
                state.xkb_state = None;
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Sperre {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if format != WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                    protokoll("Tastaturbelegung in unbekanntem Format");
                    return;
                }
                // SAFETY: fd und size stammen vom Compositor (wl_keyboard.keymap).
                let keymap = unsafe {
                    xkb::Keymap::new_from_fd(
                        &state.xkb_context,
                        fd,
                        size as usize,
                        xkb::KEYMAP_FORMAT_TEXT_V1,
                        xkb::KEYMAP_COMPILE_NO_FLAGS,
                    )
                };
                match keymap {
                    Ok(Some(keymap)) => state.xkb_state = Some(xkb::State::new(&keymap)),
                    _ => protokoll("Tastaturbelegung nicht ladbar"),
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(s) = &mut state.xkb_state {
                    s.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                    state.neu_zeichnen = true;
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => {
                let Some(s) = &state.xkb_state else { return };
                let code = xkb::Keycode::new(key + 8);
                let keysym = s.key_get_one_sym(code).raw();
                let text = s.key_get_utf8(code);
                let strg = s.mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE);
                state.taste_gedrueckt(keysym, strg, &text);
            }
            _ => {}
        }
    }
}

delegate_noop!(Sperre: ignore wl_compositor::WlCompositor);
delegate_noop!(Sperre: ignore wl_shm::WlShm);
delegate_noop!(Sperre: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Sperre: ignore wl_surface::WlSurface);
delegate_noop!(Sperre: ignore ext_session_lock_manager_v1::ExtSessionLockManagerV1);

/// Sperrt die Sitzung und kehrt erst nach dem Entsperren zurück.
/// Exit-Status: 0 entsperrt (oder vom Compositor beendet), 2 Sperre
/// abgelehnt oder Protokoll fehlt, 1 anderer Fehler.
pub fn ausfuehren() -> i32 {
    match sperren() {
        Ok(code) => code,
        Err(e) => {
            protokoll(&format!("Sperre fehlgeschlagen: {e}"));
            1
        }
    }
}

fn sperren() -> Result<i32, String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("kein Wayland: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init::<Sperre>(&conn).map_err(|e| format!("Registry: {e}"))?;
    let qh = queue.handle();
    let compositor: wl_compositor::WlCompositor = globals
        .bind(&qh, 4..=6, ())
        .map_err(|e| format!("wl_compositor: {e}"))?;
    let shm: wl_shm::WlShm = globals
        .bind(&qh, 1..=1, ())
        .map_err(|e| format!("wl_shm: {e}"))?;
    let manager: ext_session_lock_manager_v1::ExtSessionLockManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .map_err(|_| "der Compositor bietet ext-session-lock-v1 nicht an".to_string())?;
    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 1..=7, ())
        .map_err(|e| format!("wl_seat: {e}"))?;
    let _ = seat;

    let benutzer = std::env::var("USER")
        .ok()
        .filter(|u| !u.is_empty())
        .or_else(benutzername_aus_uid)
        .ok_or_else(|| "Benutzername unbekannt (USER)".to_string())?;
    let testpasswort = testpasswort(
        crate::aktion::trockenlauf(),
        std::env::var("XDG_RUNTIME_DIR").ok().as_deref(),
        std::env::var("QS_TESTPASSWORT").ok().as_deref(),
    );
    let logo = cairo::ImageSurface::create_from_png(&mut &LOGO_PNG[..]).ok();
    let dienst = pam::dienst();

    let mut weck = [0 as libc::c_int; 2];
    // SAFETY: pipe2 mit gültigem Feld; die Deskriptoren gehören danach OwnedFd.
    let (weck_leser, weck_schreiber) = unsafe {
        if libc::pipe2(weck.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) < 0 {
            return Err("pipe2 fehlgeschlagen".to_string());
        }
        (OwnedFd::from_raw_fd(weck[0]), OwnedFd::from_raw_fd(weck[1]))
    };

    let mut state = Sperre {
        compositor,
        shm,
        lock: None,
        keyboard: None,
        ausgaenge: Vec::new(),
        xkb_context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
        xkb_state: None,
        logo,
        benutzer,
        dienst,
        testpasswort,
        passwort: String::new(),
        meldung: None,
        pruefung: None,
        weck_schreiber: Some(weck_schreiber),
        gesperrt: false,
        fertig: None,
        neu_zeichnen: false,
        uhr: zeit::jetzt(),
    };
    protokoll(&format!(
        "Sperre: Benutzer {}, PAM-Dienst {}{}",
        state.benutzer,
        state.dienst,
        if state.testpasswort.is_some() {
            ", Testpasswort (Trockenlauf)"
        } else {
            ""
        }
    ));

    // Vorhandene Ausgänge binden.
    let outputs: Vec<(u32, u32)> = globals.contents().with_list(|liste| {
        liste
            .iter()
            .filter(|g| g.interface == "wl_output")
            .map(|g| (g.name, g.version))
            .collect()
    });
    let registry = globals.registry();
    for (name, version) in outputs {
        let output = registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), &qh, name);
        state.ausgaenge.push(Ausgang {
            global: name,
            output,
            name: String::new(),
            surface: None,
            lock_surface: None,
            breite: 0,
            hoehe: 0,
            puffer: Vec::new(),
        });
    }
    if state.ausgaenge.is_empty() {
        return Err("kein Ausgang (wl_output) vorhanden".to_string());
    }
    queue
        .roundtrip(&mut state)
        .map_err(|e| format!("Roundtrip: {e}"))?;

    // Sperren, dann je Ausgang eine Sperrfläche.
    state.lock = Some(manager.lock(&qh, ()));
    let globals_der_ausgaenge: Vec<u32> = state.ausgaenge.iter().map(|a| a.global).collect();
    for g in globals_der_ausgaenge {
        state.sperrflaeche(g, &qh);
    }
    conn.flush().map_err(|e| format!("Flush: {e}"))?;

    let wl_fd = conn.as_fd().as_raw_fd();
    let mut naechste_uhr = Instant::now() + Duration::from_millis(zeit::bis_zur_naechsten_minute());
    loop {
        if let Some(code) = state.fertig {
            queue.roundtrip(&mut state).ok();
            if code == 0 && !state.gesperrt {
                protokoll("Sperre beendet, ohne gesperrt gewesen zu sein");
            }
            return Ok(code);
        }
        if state.neu_zeichnen {
            state.alle_zeichnen(&qh);
        }
        conn.flush().map_err(|e| format!("Flush: {e}"))?;
        let Some(leser) = queue.prepare_read() else {
            queue
                .dispatch_pending(&mut state)
                .map_err(|e| format!("Dispatch: {e}"))?;
            continue;
        };
        let jetzt = Instant::now();
        let timeout = naechste_uhr.saturating_duration_since(jetzt).as_millis() as libc::c_int;
        let mut fds = [
            libc::pollfd {
                fd: wl_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: weck_leser.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: poll mit zwei gültigen pollfd-Einträgen.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout.max(0)) };
        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                return Err(format!("poll: {err}"));
            }
        }
        if fds[0].revents & libc::POLLIN != 0 {
            if let Err(e) = leser.read()
                && !matches!(e, wayland_client::backend::WaylandError::Io(ref io) if io.kind() == std::io::ErrorKind::WouldBlock)
            {
                return Err(format!("Wayland-Verbindung: {e}"));
            }
        } else {
            drop(leser);
        }
        queue
            .dispatch_pending(&mut state)
            .map_err(|e| format!("Dispatch: {e}"))?;
        if fds[1].revents & libc::POLLIN != 0 {
            let mut puffer = [0u8; 16];
            // SAFETY: nichtblockierendes Lesen aus der Weckpipe in einen Puffer.
            unsafe {
                while libc::read(
                    weck_leser.as_raw_fd(),
                    puffer.as_mut_ptr() as *mut libc::c_void,
                    puffer.len(),
                ) > 0
                {}
            }
            state.pruefung_abholen();
        }
        if Instant::now() >= naechste_uhr {
            state.uhr = zeit::jetzt();
            state.neu_zeichnen = true;
            naechste_uhr = Instant::now() + Duration::from_millis(zeit::bis_zur_naechsten_minute());
        }
    }
}

fn benutzername_aus_uid() -> Option<String> {
    // SAFETY: getpwuid liefert einen statischen Eintrag oder NULL.
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_name.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xkb::keysyms as k;

    #[test]
    fn testpasswort_nur_isoliert_im_trockenlauf() {
        assert_eq!(
            testpasswort(true, Some("/home/x/laufzeit/qed-runtime-1"), Some("geheim")),
            Some("geheim".to_string())
        );
        // echte Sitzung: nie
        assert_eq!(
            testpasswort(true, Some("/run/user/1001"), Some("geheim")),
            None
        );
        // ohne Trockenlauf: nie
        assert_eq!(
            testpasswort(false, Some("/tmp/qed-runtime-1"), Some("geheim")),
            None
        );
        assert_eq!(testpasswort(true, None, Some("geheim")), None);
        assert_eq!(testpasswort(true, Some(""), Some("geheim")), None);
        assert_eq!(testpasswort(true, Some("/tmp/x"), Some("")), None);
        assert_eq!(testpasswort(true, Some("/tmp/x"), None), None);
    }

    #[test]
    fn tasten_im_passwortfeld() {
        assert_eq!(taste(k::KEY_Return, false, "\r"), Taste::Pruefen);
        assert_eq!(taste(k::KEY_KP_Enter, false, "\r"), Taste::Pruefen);
        assert_eq!(taste(k::KEY_BackSpace, false, "\u{8}"), Taste::Loeschen);
        assert_eq!(taste(k::KEY_Escape, false, "\u{1b}"), Taste::Leeren);
        assert_eq!(taste(k::KEY_u, true, "\u{15}"), Taste::Leeren);
        assert_eq!(taste(k::KEY_a, false, "a"), Taste::Zeichen("a".to_string()));
        assert_eq!(
            taste(k::KEY_adiaeresis, false, "ä"),
            Taste::Zeichen("ä".to_string())
        );
        assert_eq!(taste(k::KEY_a, true, "\u{1}"), Taste::Nichts);
        assert_eq!(taste(k::KEY_Shift_L, false, ""), Taste::Nichts);
        assert_eq!(taste(k::KEY_Tab, false, "\t"), Taste::Nichts);
    }

    #[test]
    fn logo_ist_ladbar() {
        let logo = cairo::ImageSurface::create_from_png(&mut &LOGO_PNG[..]).unwrap();
        assert_eq!(logo.width(), 128);
        assert_eq!(logo.height(), 128);
    }

    #[test]
    fn malen_laeuft_in_beiden_aufloesungen() {
        for (b, h) in [(1024, 768), (1920, 1080), (640, 480)] {
            let bild = cairo::ImageSurface::create(cairo::Format::ARgb32, b, h).unwrap();
            let cr = cairo::Context::new(&bild).unwrap();
            malen(
                &cr,
                b as f64,
                h as f64,
                &Inhalt {
                    uhr: &("12:34".to_string(), "Samstag, 18. Oktober 2026".to_string()),
                    benutzer: "user",
                    passwort_laenge: 5,
                    meldung: Some(&("Falsches Passwort".to_string(), true)),
                    prueft: false,
                    feststell: true,
                    logo: cairo::ImageSurface::create_from_png(&mut &LOGO_PNG[..])
                        .ok()
                        .as_ref(),
                },
            );
            assert_eq!(cr.status(), Ok(()));
        }
    }
}
