//! Leerlaufdienst: `querencia-sitzung --leerlauf MINUTEN` wartet über
//! ext-idle-notify-v1 (labwc 0.9.8 bietet es an) auf Leerlauf des Sitzes
//! und startet dann `querencia-sitzung --sperren --sofort`. Läuft der
//! Sperrbildschirm noch (oder ist die Sitzung schon gesperrt, dann endet
//! der neue Sperrprozess mit `finished`), wird kein zweiter gestartet.
//! Endet mit der Wayland-Verbindung (Sitzungsende).
//!
//! Jeder Sperrprozess bekommt einen Warte-Thread, der ihn mit `wait()`
//! erntet, sobald er endet (QD-050): früher geschah das nur per `try_wait`
//! beim nächsten Leerlauf-Ereignis, und weil „Eingabe erkannt“ schon während
//! der Sperre kommt, blieb nach dem Entsperren bis zum nächsten Leerlauf ein
//! Zombie `[querencia-sitzu] <defunct>` stehen.
//!
//! Seit 0.1.4 bietet der Dienst `org.freedesktop.ScreenSaver` an
//! (`schoner.rs`, QD-104). Solange eine Hemmung gilt, sperrt der Leerlauf
//! nicht; fällt die letzte weg, zählt der Leerlauf von vorn. Dafür läuft
//! der Dienst in einer GLib-Hauptschleife, die den Wayland-Socket und den
//! Sitzungsbus gemeinsam bedient.
//!
//! Seit 0.1.5 schaltet der Dienst außerdem den Dunkelmodus für fremde
//! GTK-3-Programme mit (`mitschalten.rs`).
//!
//! Seit 0.1.6 schaltet er die Ausgänge ab, wenn die Sitzung gesperrt ist
//! und `BildschirmAus` (Vorgabe 120 s, `sitzung.ini`, 0 = nie) lang keine
//! Eingabe kam (`bildschirm.rs`). Dafür legt er mit der Sperre eine zweite
//! Leerlauf-Benachrichtigung an; ihr `resumed` (Eingabe) schaltet die
//! Ausgänge wieder an, ebenso das Entsperren. Gilt eine Hemmung über
//! `org.freedesktop.ScreenSaver` (Video), bleiben die Ausgänge an; eine
//! Wayland-Hemmung (idle-inhibit) hält labwc selbst gegen beide
//! Benachrichtigungen.

use std::cell::RefCell;
use std::io::ErrorKind;
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Stdio};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use gtk::gio;
use gtk::glib;
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_output, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};
use wayland_protocols_wlr::output_power_management::v1::client::{
    zwlr_output_power_manager_v1, zwlr_output_power_v1,
};

use crate::aktion::{eigenes_programm, protokoll};
use crate::aufraeumen;
use crate::bildschirm::{self, Leistung};
use crate::einstellung;
use crate::schoner::{self, Hemmungen};

/// Alle so viele Sekunden sieht der Dienst nach, ob ein Sperrbildschirm
/// läuft, den er nicht selbst gestartet hat (Tastenkürzel), für
/// `ActiveChanged`.
const FREMDE_SPERRE_PRUEFEN_S: u32 = 5;

/// Wofür eine Leerlauf-Benachrichtigung da ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Art {
    /// Leerlauf → Sperre
    Sperre,
    /// gesperrt und Leerlauf → Bildschirm aus (0.1.6)
    Bildschirm,
}

/// Die Leerlauf-Benachrichtigung am Compositor; neu angelegt zählt sie den
/// Leerlauf von vorn.
struct Melder {
    conn: Connection,
    qh: QueueHandle<Leerlauf>,
    notifier: ext_idle_notifier_v1::ExtIdleNotifierV1,
    seat: wl_seat::WlSeat,
    ms: u32,
    benachrichtigung: ext_idle_notification_v1::ExtIdleNotificationV1,
}

impl Melder {
    fn neu_zaehlen(&mut self) {
        self.benachrichtigung.destroy();
        self.benachrichtigung =
            self.notifier
                .get_idle_notification(self.ms, &self.seat, &self.qh, Art::Sperre);
        self.flush();
    }

    /// Eine weitere Benachrichtigung nach `ms` ohne Eingabe.
    fn bildschirm(&self, ms: u32) -> ext_idle_notification_v1::ExtIdleNotificationV1 {
        let b = self
            .notifier
            .get_idle_notification(ms, &self.seat, &self.qh, Art::Bildschirm);
        self.flush();
        b
    }

    fn flush(&self) {
        if let Err(e) = self.conn.flush() {
            protokoll(&format!("Leerlauf: Wayland-Flush: {e}"));
        }
    }
}

struct Leerlauf {
    /// `true`, solange der zuletzt gestartete Sperrprozess läuft; sein
    /// Warte-Thread setzt es nach dem Ernten zurück.
    sperre: Option<Arc<AtomicBool>>,
    gesperrt_mal: u32,
    hemmungen: Hemmungen,
    /// Ohne Compositor (Unit-Tests) `None`.
    melder: Option<Melder>,
    /// Sitzungsbus für `ActiveChanged`, wenn der Dienst angeboten wird.
    bus: Option<gio::DBusConnection>,
    /// Zuletzt gemeldeter Sperrzustand und seit wann.
    gesperrt_gemeldet: bool,
    gesperrt_ab: Option<Instant>,
    /// Ausgänge und ihre Leistungssteuerung (0.1.6).
    leistung: Leistung,
    /// Bildschirm aus nach so vielen ms gesperrt ohne Eingabe; 0 = nie.
    aus_nach_ms: u32,
    /// Läuft, solange gesperrt ist und `aus_nach_ms` > 0.
    bildschirm_melder: Option<ext_idle_notification_v1::ExtIdleNotificationV1>,
}

impl Leerlauf {
    fn neu(melder: Option<Melder>) -> Self {
        Self {
            sperre: None,
            gesperrt_mal: 0,
            hemmungen: Hemmungen::default(),
            melder,
            bus: None,
            gesperrt_gemeldet: false,
            gesperrt_ab: None,
            leistung: Leistung::neu(None),
            aus_nach_ms: 0,
            bildschirm_melder: None,
        }
    }

    /// `true`, wenn noch ein Sperrprozess läuft (geerntet wird im Warte-Thread).
    fn sperre_laeuft(&mut self) -> bool {
        let laeuft = self
            .sperre
            .as_ref()
            .is_some_and(|l| l.load(Ordering::Acquire));
        if !laeuft {
            self.sperre = None;
        }
        laeuft
    }

    /// Eigener Sperrprozess oder einer, den das Tastenkürzel gestartet hat.
    fn irgendeine_sperre(&mut self) -> bool {
        self.sperre_laeuft() || fremde_sperre()
    }

    /// Meldet `ActiveChanged`, wenn sich der Sperrzustand geändert hat.
    fn zustand_pruefen(&mut self) -> bool {
        let jetzt = self.irgendeine_sperre();
        if jetzt != self.gesperrt_gemeldet {
            self.gesperrt_gemeldet = jetzt;
            self.gesperrt_ab = jetzt.then(Instant::now);
            protokoll(&format!(
                "ScreenSaver: ActiveChanged({jetzt}){}",
                if self.bus.is_some() {
                    ""
                } else {
                    " (ohne Bus)"
                }
            ));
            if let Some(bus) = &self.bus {
                schoner::aktiv_melden(bus, jetzt);
            }
            if jetzt {
                self.bildschirm_zaehlen("gesperrt");
            } else {
                self.bildschirm_ende("entsperrt");
            }
        }
        jetzt
    }

    /// Zählt „Bildschirm aus“ von vorn (neue Benachrichtigung), wenn
    /// gesperrt ist und die Ausgänge abschaltbar sind.
    fn bildschirm_zaehlen(&mut self, warum: &str) {
        if let Some(alt) = self.bildschirm_melder.take() {
            alt.destroy();
        }
        if self.aus_nach_ms == 0 || !self.leistung.verfuegbar() {
            return;
        }
        if let Some(m) = &self.melder {
            self.bildschirm_melder = Some(m.bildschirm(self.aus_nach_ms));
            protokoll(&format!(
                "Bildschirm: aus nach {} s ohne Eingabe ({warum})",
                self.aus_nach_ms / 1000
            ));
        }
    }

    /// Nach dem Entsperren: keine Abschaltung mehr, Ausgänge an.
    fn bildschirm_ende(&mut self, warum: &str) {
        if let Some(alt) = self.bildschirm_melder.take() {
            alt.destroy();
            if let Some(m) = &self.melder {
                m.flush();
            }
        }
        self.bildschirm_an(warum);
    }

    fn bildschirm_an(&mut self, warum: &str) {
        if self.leistung.einschalten() {
            if let Some(m) = &self.melder {
                m.flush();
            }
            protokoll(&format!("Bildschirm: Ausgänge an ({warum})"));
        }
    }

    /// Ausgänge aus, wenn noch gesperrt und nichts hemmt.
    fn bildschirm_aus(&mut self) {
        if !self.irgendeine_sperre() {
            return;
        }
        if self.hemmungen.aktiv() {
            protokoll(&format!(
                "Bildschirm bleibt an: {} Hemmung(en): {}",
                self.hemmungen.anzahl(),
                self.hemmungen.beschreibung()
            ));
            return;
        }
        let Some(m) = &self.melder else {
            return;
        };
        let qh = m.qh.clone();
        if self.leistung.ausschalten(&qh) {
            m.flush();
            protokoll(&format!(
                "Bildschirm: {} Ausgang/Ausgänge aus (gesperrt, keine Eingabe)",
                self.leistung.anzahl()
            ));
        }
    }

    /// Startet den Sperrbildschirm, wenn keiner läuft; `anlass` fürs
    /// Protokoll. `true`, wenn danach einer läuft.
    fn sperre_starten(&mut self, anlass: &str) -> bool {
        if self.sperre_laeuft() {
            protokoll(&format!("{anlass}: Sperrbildschirm läuft bereits"));
            return true;
        }
        if fremde_sperre() {
            protokoll(&format!(
                "{anlass}: Sperrbildschirm läuft bereits (Tastenkürzel)"
            ));
            return true;
        }
        let programm = eigenes_programm();
        match Command::new(&programm)
            .args(["--sperren", "--sofort"])
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(kind) => {
                self.gesperrt_mal += 1;
                protokoll(&format!(
                    "{anlass}: {} --sperren --sofort gestartet (PID {}, Nr. {})",
                    programm.display(),
                    kind.id(),
                    self.gesperrt_mal
                ));
                self.sperre = Some(ernten(kind, anstossen));
                self.zustand_pruefen();
                true
            }
            Err(e) => {
                protokoll(&format!("{anlass}: Sperre nicht startbar: {e}"));
                false
            }
        }
    }

    fn neu_zaehlen(&mut self, warum: &str) {
        if let Some(m) = self.melder.as_mut() {
            m.neu_zaehlen();
            protokoll(&format!("Leerlauf zählt von vorn ({warum})"));
        }
    }
}

impl schoner::Sitzung for Leerlauf {
    fn gehemmt(&mut self, erste: bool) {
        if erste {
            protokoll("Leerlaufsperre gehemmt");
        }
        self.bildschirm_an("Hemmung");
    }

    fn frei(&mut self) {
        protokoll("keine Hemmung mehr");
        self.neu_zaehlen("letzte Hemmung aufgehoben");
        if self.gesperrt_gemeldet {
            self.bildschirm_zaehlen("letzte Hemmung aufgehoben");
        }
    }

    fn sperren(&mut self, anlass: &str) -> bool {
        self.sperre_starten(anlass)
    }

    fn gesperrt(&mut self) -> bool {
        self.zustand_pruefen()
    }

    fn gesperrt_seit(&mut self) -> u32 {
        self.zustand_pruefen();
        self.gesperrt_ab
            .map_or(0, |t| t.elapsed().as_secs().min(u32::MAX as u64) as u32)
    }

    fn aktivitaet(&mut self) {
        if !self.irgendeine_sperre() {
            self.neu_zaehlen("SimulateUserActivity");
        } else if self.gesperrt_gemeldet {
            self.bildschirm_an("SimulateUserActivity");
            self.bildschirm_zaehlen("SimulateUserActivity");
        }
    }

    fn hemmungen(&mut self) -> &mut Hemmungen {
        &mut self.hemmungen
    }
}

/// Läuft ein Sperrbildschirm dieser Sitzung, den nicht dieser Dienst
/// gestartet hat?
fn fremde_sperre() -> bool {
    let Some(labwc) = std::env::var("LABWC_PID")
        .ok()
        .and_then(|p| p.parse::<u32>().ok())
    else {
        return false;
    };
    // SAFETY: getuid(2) hat keine Vorbedingungen.
    let uid = unsafe { libc::getuid() };
    !aufraeumen::sperren_finden(
        std::path::Path::new("/proc"),
        uid,
        labwc,
        std::process::id(),
    )
    .is_empty()
}

thread_local! {
    /// Der Dienst im Hauptthread, für [`anstossen`].
    static DIENST: RefCell<Weak<RefCell<Leerlauf>>> = const { RefCell::new(Weak::new()) };
}

/// Aus dem Warte-Thread: den Hauptthread den Sperrzustand neu prüfen lassen
/// (`ActiveChanged(false)` gleich nach dem Entsperren).
fn anstossen() {
    glib::MainContext::default().invoke(|| {
        DIENST.with(|d| {
            if let Some(dienst) = d.borrow().upgrade()
                && let Ok(mut dienst) = dienst.try_borrow_mut()
            {
                dienst.zustand_pruefen();
            }
        })
    });
}

/// Erntet `kind` in einem eigenen Thread, sobald er endet, ruft danach
/// `nachher` und meldet das über die zurückgegebene Marke (`true` = läuft
/// noch). Ist kein Thread startbar, gilt der Prozess als beendet, damit die
/// Leerlaufsperre nicht dauerhaft blockiert; er bleibt dann bis zum Ende
/// des Dienstes ungeerntet.
fn ernten(mut kind: Child, nachher: impl FnOnce() + Send + 'static) -> Arc<AtomicBool> {
    let laeuft = Arc::new(AtomicBool::new(true));
    let signal = laeuft.clone();
    let pid = kind.id();
    let thread = std::thread::Builder::new()
        .name("sperre-ernten".into())
        .spawn(move || {
            match kind.wait() {
                Ok(status) => protokoll(&format!(
                    "Sperrbildschirm beendet mit {status} (PID {pid}, geerntet)"
                )),
                Err(e) => protokoll(&format!("Sperrprozess {pid} nicht prüfbar: {e}")),
            }
            signal.store(false, Ordering::Release);
            nachher();
        });
    if let Err(e) = thread {
        protokoll(&format!("Leerlauf: Warte-Thread nicht startbar: {e}"));
        laeuft.store(false, Ordering::Release);
    }
    laeuft
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, Art> for Leerlauf {
    fn event(
        state: &mut Self,
        _: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        art: &Art,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if *art == Art::Bildschirm {
            match event {
                ext_idle_notification_v1::Event::Idled => state.bildschirm_aus(),
                ext_idle_notification_v1::Event::Resumed => state.bildschirm_an("Eingabe"),
                _ => {}
            }
            return;
        }
        match event {
            ext_idle_notification_v1::Event::Idled => {
                protokoll("Leerlauf erreicht");
                if state.hemmungen.aktiv() {
                    protokoll(&format!(
                        "Leerlauf: Sperre gehemmt durch {} Hemmung(en): {}",
                        state.hemmungen.anzahl(),
                        state.hemmungen.beschreibung()
                    ));
                } else {
                    state.sperre_starten("Leerlauf");
                }
            }
            ext_idle_notification_v1::Event::Resumed => {
                protokoll("Eingabe erkannt, Leerlauf beendet");
                state.sperre_laeuft();
            }
            _ => {}
        }
    }
}

/// Ausgänge kommen und gehen (Anstecken, Abschalten durch labwc).
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Leerlauf {
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
                let a = registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
                state.leistung.ausgang_dazu(name, a);
            }
            wl_registry::Event::GlobalRemove { name } => state.leistung.ausgang_weg(name),
            _ => {}
        }
    }
}

impl Dispatch<zwlr_output_power_v1::ZwlrOutputPowerV1, ()> for Leerlauf {
    fn event(
        _: &mut Self,
        _: &zwlr_output_power_v1::ZwlrOutputPowerV1,
        event: zwlr_output_power_v1::Event,
        _: &(),
        conn: &Connection,
        _: &QueueHandle<Self>,
    ) {
        bildschirm::ereignis(event, conn);
    }
}

delegate_noop!(Leerlauf: ignore wl_seat::WlSeat);
delegate_noop!(Leerlauf: ignore wl_output::WlOutput);
delegate_noop!(Leerlauf: ignore ext_idle_notifier_v1::ExtIdleNotifierV1);
delegate_noop!(Leerlauf: ignore zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1);

/// Läuft bis zum Ende der Wayland-Verbindung; Exit 2 ohne Protokoll.
pub fn ausfuehren(sekunden: u32) -> i32 {
    match dienst(sekunden) {
        Ok(()) => 0,
        Err(e) => {
            protokoll(&format!("Leerlaufdienst: {e}"));
            2
        }
    }
}

/// Liest und verteilt, was am Wayland-Socket anliegt (ohne zu blockieren).
fn wayland_bedienen(
    queue: &mut wayland_client::EventQueue<Leerlauf>,
    conn: &Connection,
    zustand: &RefCell<Leerlauf>,
) -> Result<(), String> {
    if let Some(lesen) = queue.prepare_read() {
        match lesen.read() {
            Ok(_) => {}
            Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => return Err(format!("Wayland-Verbindung beendet: {e}")),
        }
    }
    queue
        .dispatch_pending(&mut zustand.borrow_mut())
        .map_err(|e| format!("Wayland-Verbindung beendet: {e}"))?;
    conn.flush()
        .map_err(|e| format!("Wayland-Verbindung beendet: {e}"))
}

fn dienst(sekunden: u32) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("kein Wayland: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init::<Leerlauf>(&conn).map_err(|e| format!("Registry: {e}"))?;
    let qh = queue.handle();
    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 1..=7, ())
        .map_err(|e| format!("wl_seat: {e}"))?;
    let notifier: ext_idle_notifier_v1::ExtIdleNotifierV1 = globals
        .bind(&qh, 1..=1, ())
        .map_err(|_| "der Compositor bietet ext-idle-notify-v1 nicht an".to_string())?;
    let ms = sekunden.saturating_mul(1000);
    let benachrichtigung = notifier.get_idle_notification(ms, &seat, &qh, Art::Sperre);
    protokoll(&format!(
        "Leerlaufdienst: sperrt nach {sekunden} s ohne Eingabe"
    ));
    // Bildschirm aus nach der Sperre (0.1.6): Ausgänge und Leistungssteuerung.
    let manager: Option<zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1> =
        globals.bind(&qh, 1..=1, ()).ok();
    let mut leistung = Leistung::neu(manager);
    for g in globals.contents().clone_list() {
        if g.interface == "wl_output" {
            let a = globals.registry().bind::<wl_output::WlOutput, _, _>(
                g.name,
                g.version.min(4),
                &qh,
                (),
            );
            leistung.ausgang_dazu(g.name, a);
        }
    }
    let (aus_s, herkunft) = einstellung::bildschirm_aus();
    if !leistung.verfuegbar() {
        protokoll(
            "Bildschirm: der Compositor bietet wlr-output-power-management nicht an, Ausgänge bleiben an",
        );
    } else if aus_s == 0 {
        protokoll(&format!(
            "Bildschirm: bleibt nach der Sperre an ({herkunft})"
        ));
    } else {
        protokoll(&format!(
            "Bildschirm: aus nach {aus_s} s gesperrt ohne Eingabe ({herkunft}; {} Ausgang/Ausgänge)",
            leistung.anzahl()
        ));
    }
    let mut leerlauf = Leerlauf::neu(Some(Melder {
        conn: conn.clone(),
        qh,
        notifier,
        seat,
        ms,
        benachrichtigung,
    }));
    leerlauf.leistung = leistung;
    leerlauf.aus_nach_ms = aus_s.saturating_mul(1000);
    let zustand = Rc::new(RefCell::new(leerlauf));
    let bus = schoner::anbieten(zustand.clone());
    zustand.borrow_mut().bus = bus;
    DIENST.with(|d| *d.borrow_mut() = Rc::downgrade(&zustand));
    // Dunkelmodus für fremde GTK-3-Programme (0.1.5, QD-103); lebt bis
    // zum Ende des Dienstes.
    let _dunkel = crate::mitschalten::starten();

    let schleife = glib::MainLoop::new(None, false);
    let fehler: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    wayland_bedienen(&mut queue, &conn, &zustand)?;
    let fd = conn.backend().poll_fd().as_raw_fd();
    {
        let zustand = zustand.clone();
        let schleife = schleife.clone();
        let fehler = fehler.clone();
        let conn = conn.clone();
        glib_unix::unix_fd_add_local(
            fd,
            glib::IOCondition::IN | glib::IOCondition::ERR | glib::IOCondition::HUP,
            move |_, bedingung: glib::IOCondition| {
                let ergebnis = wayland_bedienen(&mut queue, &conn, &zustand).and_then(|()| {
                    if bedingung.intersects(glib::IOCondition::ERR | glib::IOCondition::HUP) {
                        Err("Wayland-Verbindung beendet (Socket geschlossen)".to_string())
                    } else {
                        Ok(())
                    }
                });
                match ergebnis {
                    Ok(()) => glib::ControlFlow::Continue,
                    Err(e) => {
                        *fehler.borrow_mut() = Some(e);
                        schleife.quit();
                        glib::ControlFlow::Break
                    }
                }
            },
        );
    }
    {
        let zustand = zustand.clone();
        glib::timeout_add_seconds_local(FREMDE_SPERRE_PRUEFEN_S, move || {
            zustand.borrow_mut().zustand_pruefen();
            glib::ControlFlow::Continue
        });
    }
    schleife.run();
    let e = fehler.borrow_mut().take();
    Err(e.unwrap_or_else(|| "Hauptschleife beendet".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Ein beendetes Kind wird sofort geerntet (kein `<defunct>`), ohne dass
    /// ein weiteres Leerlauf-Ereignis kommt (QD-050).
    #[test]
    fn beendetes_kind_wird_geerntet() {
        let kind = Command::new("sh")
            .args(["-c", "exit 0"])
            .stdin(Stdio::null())
            .spawn()
            .expect("sh startbar");
        let pid = kind.id();
        let (tx, rx) = std::sync::mpsc::channel();
        let laeuft = ernten(kind, move || tx.send(()).unwrap());
        let start = Instant::now();
        while laeuft.load(Ordering::Acquire) {
            assert!(start.elapsed() < Duration::from_secs(5), "nicht geerntet");
            std::thread::sleep(Duration::from_millis(10));
        }
        // nach dem Ernten wird `nachher` gerufen
        rx.recv_timeout(Duration::from_secs(5))
            .expect("nachher gerufen");
        // Nach dem Ernten gibt es den Prozesseintrag nicht mehr (auch keinen Zombie).
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }

    /// Mehrfach „sperren und entsperren“ mit einer Sperr-Attrappe (kurzer
    /// `sleep` statt `--sperren --sofort`) über denselben Weg wie der Dienst:
    /// danach hat dieser Prozess kein Zombie-Kind (`ps` ohne `<defunct>`).
    #[test]
    fn mehrfach_sperren_hinterlaesst_keinen_zombie() {
        let mut leerlauf = Leerlauf::neu(None);
        let mut pids = Vec::new();
        for _ in 0..5 {
            assert!(!leerlauf.sperre_laeuft(), "Attrappe läuft noch");
            let kind = Command::new("sleep")
                .arg("0.2")
                .stdin(Stdio::null())
                .spawn()
                .expect("sleep startbar");
            pids.push(kind.id());
            leerlauf.sperre = Some(ernten(kind, || {}));
            leerlauf.gesperrt_mal += 1;
            let start = Instant::now();
            while leerlauf.sperre_laeuft() {
                assert!(start.elapsed() < Duration::from_secs(10), "nicht geerntet");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        assert_eq!(leerlauf.gesperrt_mal, 5);
        let ps = Command::new("ps")
            .args([
                "-o",
                "pid=,stat=",
                "--ppid",
                &std::process::id().to_string(),
            ])
            .output()
            .expect("ps startbar");
        let kinder = String::from_utf8_lossy(&ps.stdout);
        for pid in &pids {
            assert!(
                !kinder
                    .lines()
                    .any(|l| l.split_whitespace().next() == Some(&pid.to_string())),
                "Attrappe {pid} noch da (Zombie?): {kinder}"
            );
        }
        assert!(
            !kinder.lines().any(|l| l
                .split_whitespace()
                .nth(1)
                .is_some_and(|s| s.starts_with('Z'))),
            "Zombie-Kind: {kinder}"
        );
    }

    #[test]
    fn laufendes_kind_gilt_als_laufend() {
        let mut leerlauf = Leerlauf::neu(None);
        assert!(!leerlauf.sperre_laeuft());
        let kind = Command::new("sleep")
            .arg("2")
            .stdin(Stdio::null())
            .spawn()
            .expect("sleep startbar");
        leerlauf.sperre = Some(ernten(kind, || {}));
        assert!(leerlauf.sperre_laeuft());
        let start = Instant::now();
        while leerlauf.sperre_laeuft() {
            assert!(start.elapsed() < Duration::from_secs(10), "nicht geerntet");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(leerlauf.sperre.is_none());
    }

    /// Sperrzustand und `GetActiveTime` folgen dem eigenen Sperrprozess
    /// (Attrappe), ohne Bus.
    #[test]
    fn sperrzustand_folgt_dem_sperrprozess() {
        use schoner::Sitzung;
        let mut leerlauf = Leerlauf::neu(None);
        assert!(!leerlauf.gesperrt());
        assert_eq!(leerlauf.gesperrt_seit(), 0);
        let kind = Command::new("sleep")
            .arg("1.5")
            .stdin(Stdio::null())
            .spawn()
            .expect("sleep startbar");
        leerlauf.sperre = Some(ernten(kind, || {}));
        assert!(leerlauf.gesperrt());
        assert!(leerlauf.gesperrt_ab.is_some());
        let start = Instant::now();
        while leerlauf.gesperrt() {
            assert!(start.elapsed() < Duration::from_secs(10), "nicht geerntet");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(leerlauf.gesperrt_seit(), 0);
        // ohne Melder: frei() und aktivitaet() tun nichts Schädliches
        leerlauf.frei();
        leerlauf.aktivitaet();
    }
}
