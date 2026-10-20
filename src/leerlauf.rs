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

use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};

use crate::aktion::{eigenes_programm, protokoll};

struct Leerlauf {
    /// `true`, solange der zuletzt gestartete Sperrprozess läuft; sein
    /// Warte-Thread setzt es nach dem Ernten zurück.
    sperre: Option<Arc<AtomicBool>>,
    gesperrt_mal: u32,
}

impl Leerlauf {
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

    fn sperren(&mut self) {
        if self.sperre_laeuft() {
            protokoll("Leerlauf: Sperrbildschirm läuft bereits");
            return;
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
                    "Leerlauf: {} --sperren --sofort gestartet (PID {}, Nr. {})",
                    programm.display(),
                    kind.id(),
                    self.gesperrt_mal
                ));
                self.sperre = Some(ernten(kind));
            }
            Err(e) => protokoll(&format!("Leerlauf: Sperre nicht startbar: {e}")),
        }
    }
}

/// Erntet `kind` in einem eigenen Thread, sobald er endet, und meldet das
/// über die zurückgegebene Marke (`true` = läuft noch). Ist kein Thread
/// startbar, gilt der Prozess als beendet, damit die Leerlaufsperre nicht
/// dauerhaft blockiert; er bleibt dann bis zum Ende des Dienstes ungeerntet.
fn ernten(mut kind: Child) -> Arc<AtomicBool> {
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
        });
    if let Err(e) = thread {
        protokoll(&format!("Leerlauf: Warte-Thread nicht startbar: {e}"));
        laeuft.store(false, Ordering::Release);
    }
    laeuft
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for Leerlauf {
    fn event(
        state: &mut Self,
        _: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => {
                protokoll("Leerlauf erreicht");
                state.sperren();
            }
            ext_idle_notification_v1::Event::Resumed => {
                protokoll("Eingabe erkannt, Leerlauf beendet");
                state.sperre_laeuft();
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Leerlauf {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(Leerlauf: ignore wl_seat::WlSeat);
delegate_noop!(Leerlauf: ignore ext_idle_notifier_v1::ExtIdleNotifierV1);

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
    let _benachrichtigung =
        notifier.get_idle_notification(sekunden.saturating_mul(1000), &seat, &qh, ());
    protokoll(&format!(
        "Leerlaufdienst: sperrt nach {sekunden} s ohne Eingabe"
    ));
    let mut state = Leerlauf {
        sperre: None,
        gesperrt_mal: 0,
    };
    loop {
        queue
            .blocking_dispatch(&mut state)
            .map_err(|e| format!("Wayland-Verbindung beendet: {e}"))?;
    }
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
        let laeuft = ernten(kind);
        let start = Instant::now();
        while laeuft.load(Ordering::Acquire) {
            assert!(start.elapsed() < Duration::from_secs(5), "nicht geerntet");
            std::thread::sleep(Duration::from_millis(10));
        }
        // Nach dem Ernten gibt es den Prozesseintrag nicht mehr (auch keinen Zombie).
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }

    #[test]
    fn laufendes_kind_gilt_als_laufend() {
        let mut leerlauf = Leerlauf {
            sperre: None,
            gesperrt_mal: 0,
        };
        assert!(!leerlauf.sperre_laeuft());
        let kind = Command::new("sleep")
            .arg("2")
            .stdin(Stdio::null())
            .spawn()
            .expect("sleep startbar");
        leerlauf.sperre = Some(ernten(kind));
        assert!(leerlauf.sperre_laeuft());
        let start = Instant::now();
        while leerlauf.sperre_laeuft() {
            assert!(start.elapsed() < Duration::from_secs(10), "nicht geerntet");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(leerlauf.sperre.is_none());
    }
}
