//! Querencia Sitzung – Abmelden, Bildschirm sperren, Neu starten,
//! Herunterfahren und Leerlaufsperre für Querencia Desktop (labwc).
//!
//! Siehe `cli.rs` für die Befehle. Dialoge: GTK 4 (`dialog.rs`);
//! Aktionen: `aktion.rs`; Sperrbildschirm: `sperre.rs` (ext-session-lock);
//! Leerlauf: `leerlauf.rs` (ext-idle-notify); Passwort: `pam.rs`.

mod aktion;
mod cli;
mod dialog;
mod leerlauf;
mod pam;
mod sperre;
mod zeit;

use cli::{Aktion, Befehl};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let befehl = match cli::parse(std::env::args().skip(1)) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("querencia-sitzung: {e}");
            std::process::exit(2);
        }
    };
    let code = match befehl {
        Befehl::Version => {
            println!("querencia-sitzung {VERSION}");
            0
        }
        Befehl::Hilfe => {
            print!("{}", cli::HILFE);
            0
        }
        Befehl::SperrenSofort => sperre::ausfuehren(),
        Befehl::Leerlauf { sekunden } => leerlauf::ausfuehren(sekunden),
        Befehl::Dialog { .. } | Befehl::Rueckfrage { .. } => {
            let aufbau = dialog::aufbau(&befehl).expect("Dialogbefehl");
            match dialog::rueckfrage(aufbau) {
                None => {
                    aktion::protokoll("abgebrochen");
                    0
                }
                Some(aktion) => match aktion::ausfuehren(aktion) {
                    Ok(()) => 0,
                    Err(e) => {
                        aktion::protokoll(&format!("{}: {e}", aktion.name()));
                        dialog::fehler_anzeigen(aktion.name(), &e);
                        1
                    }
                },
            }
        }
    };
    let _ = Aktion::Abmelden; // alle Varianten werden in dialog/aktion verwendet
    std::process::exit(code);
}
