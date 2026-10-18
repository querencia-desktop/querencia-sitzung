//! Leerlaufdienst: `querencia-sitzung --leerlauf MINUTEN` wartet über
//! ext-idle-notify-v1 (labwc 0.9.8 bietet es an) auf Leerlauf des Sitzes
//! und startet dann `querencia-sitzung --sperren --sofort`. Läuft der
//! Sperrbildschirm noch (oder ist die Sitzung schon gesperrt, dann endet
//! der neue Sperrprozess mit `finished`), wird kein zweiter gestartet.
//! Endet mit der Wayland-Verbindung (Sitzungsende).

use std::process::{Child, Command, Stdio};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};

use crate::aktion::{eigenes_programm, protokoll};

struct Leerlauf {
    sperre: Option<Child>,
    gesperrt_mal: u32,
}

impl Leerlauf {
    /// Räumt einen beendeten Sperrprozess ab; `true`, wenn noch einer läuft.
    fn sperre_laeuft(&mut self) -> bool {
        if let Some(kind) = &mut self.sperre {
            match kind.try_wait() {
                Ok(Some(status)) => {
                    protokoll(&format!("Sperrbildschirm beendet mit {status}"));
                    self.sperre = None;
                    false
                }
                Ok(None) => true,
                Err(e) => {
                    protokoll(&format!("Sperrprozess nicht prüfbar: {e}"));
                    self.sperre = None;
                    false
                }
            }
        } else {
            false
        }
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
                self.sperre = Some(kind);
            }
            Err(e) => protokoll(&format!("Leerlauf: Sperre nicht startbar: {e}")),
        }
    }
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
