//! Querencia Sitzung – Abmelden, Bildschirm sperren, Neu starten,
//! Herunterfahren und Leerlaufsperre für Querencia Desktop (labwc).
//!
//! Siehe `cli.rs` für die Befehle. Dialoge: GTK 4 (`dialog.rs`);
//! Aktionen: `aktion.rs`; Sperrbildschirm: `sperre.rs` (ext-session-lock);
//! Leerlauf: `leerlauf.rs` (ext-idle-notify) mit dem Dienst
//! `org.freedesktop.ScreenSaver` (`schoner.rs`) und dem Dunkelmodus für
//! fremde GTK-3-Programme (`mitschalten.rs`) und „Bildschirm aus“ nach der
//! Sperre (`bildschirm.rs`); Passwort: `pam.rs`; OpenGL-Renderer für den
//! Sitzungsstarter: `grafik.rs`.

mod aktion;
mod aufraeumen;
mod bildschirm;
mod cli;
mod dialog;
mod einstellung;
mod farbschema;
mod grafik;
mod i18n;
mod leerlauf;
mod mitschalten;
mod pam;
mod schoner;
mod sperre;
mod zeit;

use cli::{Aktion, Befehl};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    i18n::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let befehl = match cli::parse(&args) {
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
        Befehl::GrafikPruefen => grafik::ausfuehren(),
        Befehl::SperrenSofort => sperre::ausfuehren(),
        Befehl::Leerlauf { sekunden } => leerlauf::ausfuehren(sekunden),
        Befehl::Dialog { .. } | Befehl::Rueckfrage { .. } => {
            let befehl = countdown_einstellen(befehl, &args);
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
                        dialog::fehler_anzeigen(&dialog::titel(aktion), &e);
                        1
                    }
                },
            }
        }
    };
    let _ = Aktion::Abmelden; // alle Varianten werden in dialog/aktion verwendet
    std::process::exit(code);
}

/// Ohne Countdown wartet jeder Dialog, bis jemand wählt (QD-062).
/// `sitzung.ini` mit `Countdown=an` schaltet ihn ein, außer die
/// Befehlszeile legt ihn fest.
fn countdown_einstellen(befehl: Befehl, args: &[String]) -> Befehl {
    if cli::countdown_ausdruecklich(args) {
        return befehl;
    }
    let (an, herkunft) = einstellung::countdown();
    if an {
        aktion::protokoll(&format!("Countdown an ({herkunft})"));
        cli::mit_countdown(befehl)
    } else {
        befehl
    }
}
