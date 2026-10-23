//! Bildschirm aus nach der Sperre (0.1.6): Der Leerlaufdienst schaltet die
//! Ausgänge über wlr-output-power-management-unstable-v1 ab (labwc 0.9.8
//! bietet `zwlr_output_power_manager_v1`; es schaltet den Ausgang dann wie
//! „deaktiviert“, `src/output.c` `handle_output_power_manager_set_mode`)
//! und bei Eingabe wieder an. Wann, entscheidet `leerlauf.rs`.
//!
//! Für jeden Ausgang entsteht beim Ausschalten eine Steuerung
//! (`get_output_power`, `set_mode(off)`); beim Einschalten bekommt sie
//! `set_mode(on)` und wird freigegeben. Ausgänge kommen und gehen über die
//! Registry (`wl_output`), solange der Dienst läuft.

use wayland_client::protocol::wl_output;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::output_power_management::v1::client::{
    zwlr_output_power_manager_v1, zwlr_output_power_v1,
};

use crate::aktion::protokoll;

/// Ausgänge der Sitzung und ihre Leistungssteuerung.
pub struct Leistung {
    manager: Option<zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1>,
    /// Registry-Name und Ausgang.
    ausgaenge: Vec<(u32, wl_output::WlOutput)>,
    /// Steuerungen, solange die Ausgänge aus sind.
    steuerungen: Vec<zwlr_output_power_v1::ZwlrOutputPowerV1>,
    aus: bool,
}

impl Leistung {
    pub fn neu(manager: Option<zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1>) -> Self {
        Self {
            manager,
            ausgaenge: Vec::new(),
            steuerungen: Vec::new(),
            aus: false,
        }
    }

    pub fn verfuegbar(&self) -> bool {
        self.manager.is_some()
    }

    pub fn anzahl(&self) -> usize {
        self.ausgaenge.len()
    }

    pub fn ausgang_dazu(&mut self, name: u32, ausgang: wl_output::WlOutput) {
        self.ausgaenge.push((name, ausgang));
    }

    pub fn ausgang_weg(&mut self, name: u32) {
        self.ausgaenge.retain(|(n, a)| {
            if *n == name && a.version() >= 3 {
                a.release();
            }
            *n != name
        });
    }

    /// Schaltet alle Ausgänge ab; `false`, wenn es nichts zu tun gab.
    pub fn ausschalten<D>(&mut self, qh: &QueueHandle<D>) -> bool
    where
        D: Dispatch<zwlr_output_power_v1::ZwlrOutputPowerV1, ()> + 'static,
    {
        let Some(manager) = &self.manager else {
            return false;
        };
        if self.aus || self.ausgaenge.is_empty() {
            return false;
        }
        for (_, ausgang) in &self.ausgaenge {
            let steuerung = manager.get_output_power(ausgang, qh, ());
            steuerung.set_mode(zwlr_output_power_v1::Mode::Off);
            self.steuerungen.push(steuerung);
        }
        self.aus = true;
        true
    }

    /// Schaltet die Ausgänge wieder an; `false`, wenn sie nicht aus waren.
    pub fn einschalten(&mut self) -> bool {
        if !self.aus {
            return false;
        }
        for steuerung in self.steuerungen.drain(..) {
            steuerung.set_mode(zwlr_output_power_v1::Mode::On);
            steuerung.destroy();
        }
        self.aus = false;
        true
    }
}

/// Protokoll der Leistungsmeldungen des Compositors.
pub fn ereignis(event: zwlr_output_power_v1::Event, _: &Connection) {
    match event {
        zwlr_output_power_v1::Event::Mode { mode } => {
            let m = match mode.into_result() {
                Ok(zwlr_output_power_v1::Mode::Off) => "aus",
                Ok(zwlr_output_power_v1::Mode::On) => "an",
                _ => "unbekannt",
            };
            protokoll(&format!("Bildschirm: Ausgang meldet {m}"));
        }
        zwlr_output_power_v1::Event::Failed => {
            protokoll("Bildschirm: Leistungssteuerung eines Ausgangs abgelehnt (failed)");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ohne `zwlr_output_power_manager_v1` (Compositor ohne das Protokoll)
    /// schaltet nichts, und Einschalten ohne vorheriges Ausschalten tut
    /// nichts.
    #[test]
    fn ohne_manager_geschieht_nichts() {
        let mut l = Leistung::neu(None);
        assert!(!l.verfuegbar());
        assert_eq!(l.anzahl(), 0);
        assert!(!l.einschalten());
        l.ausgang_weg(7);
        assert_eq!(l.anzahl(), 0);
    }
}
