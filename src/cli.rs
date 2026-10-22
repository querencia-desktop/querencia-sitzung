//! Befehlszeile von `querencia-sitzung`: genau ein Befehl je Aufruf.
//!
//! ```text
//! querencia-sitzung --dialog [--countdown SEK]
//! querencia-sitzung --abmelden | --neustart | --herunterfahren
//!                   [--countdown SEK]
//! querencia-sitzung --sperren [--sofort]
//! querencia-sitzung --leerlauf MINUTEN | --leerlauf-sekunden SEK
//! querencia-sitzung --version | --hilfe
//! ```
//!
//! Die Rückfrage zu `--herunterfahren` bietet wie in GNOME 2 auch „Neu
//! starten“ an (0.1.7); Vorgabe und Countdown bleiben beim Herunterfahren.
//!
//! Kein Dialog zählt von selbst herunter: Sie warten, bis jemand wählt
//! (Entscheidung des Besitzers, 20.10.2026, QD-062). Den Countdown gibt es nur
//! auf Wunsch: `--countdown SEK` oder `[Dialog] Countdown=an` in
//! `sitzung.ini` (dann 60 s wie GNOME 2 für die Rückfragen).

use std::fmt;

/// Was die Sitzung tun soll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aktion {
    Abmelden,
    Sperren,
    Neustart,
    Herunterfahren,
}

impl Aktion {
    /// Name im Infinitiv, für Meldungen und Protokolle.
    pub fn name(self) -> &'static str {
        match self {
            Aktion::Abmelden => "Abmelden",
            Aktion::Sperren => "Bildschirm sperren",
            Aktion::Neustart => "Neu starten",
            Aktion::Herunterfahren => "Herunterfahren",
        }
    }
}

/// Der geparste Aufruf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Befehl {
    /// Gesamtdialog mit allen Schaltflächen; `countdown` zählt bis Abmelden.
    Dialog {
        countdown: Option<u32>,
    },
    /// Eine Aktion mit Rückfrage; `countdown` zählt bis zu dieser Aktion.
    Rueckfrage {
        aktion: Aktion,
        countdown: Option<u32>,
    },
    /// `--sperren --sofort`: Sperrbildschirm ohne Rückfrage.
    SperrenSofort,
    /// Leerlaufdienst, sperrt nach `sekunden` ohne Eingabe.
    Leerlauf {
        sekunden: u32,
    },
    Version,
    Hilfe,
}

/// Countdown wie in GNOME 2.
pub const COUNTDOWN_STANDARD: u32 = 60;
/// Länger als eine Stunde zählt kein Dialog.
pub const COUNTDOWN_MAX: u32 = 3600;
/// Leerlauf höchstens ein Tag.
pub const LEERLAUF_MAX_MINUTEN: u32 = 24 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fehler(pub String);

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn fehler<T>(text: impl Into<String>) -> Result<T, Fehler> {
    Err(Fehler(text.into()))
}

/// Zahl aus `wert` für die Option `option`, zwischen 1 und `max`.
fn zahl(option: &str, wert: Option<&str>, max: u32) -> Result<u32, Fehler> {
    let Some(wert) = wert else {
        return fehler(format!("{option} braucht eine Zahl"));
    };
    match wert.parse::<u32>() {
        Ok(n) if (1..=max).contains(&n) => Ok(n),
        _ => fehler(format!(
            "{option}: „{wert}“ ist keine Zahl zwischen 1 und {max}"
        )),
    }
}

/// Zerlegt die Argumente (ohne Programmnamen).
pub fn parse<I, S>(args: I) -> Result<Befehl, Fehler>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|a| a.as_ref().to_string()).collect();
    let mut befehle: Vec<&str> = Vec::new();
    let mut aktion = None;
    let mut sofort = false;
    let mut countdown: Option<u32> = None;
    let mut leerlauf = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let naechstes = args.get(i + 1).map(String::as_str);
        match arg {
            "--dialog" => befehle.push(arg),
            "--abmelden" => {
                befehle.push(arg);
                aktion = Some(Aktion::Abmelden);
            }
            "--sperren" => {
                befehle.push(arg);
                aktion = Some(Aktion::Sperren);
            }
            "--neustart" => {
                befehle.push(arg);
                aktion = Some(Aktion::Neustart);
            }
            "--herunterfahren" => {
                befehle.push(arg);
                aktion = Some(Aktion::Herunterfahren);
            }
            "--leerlauf" => {
                befehle.push(arg);
                leerlauf = Some(zahl(arg, naechstes, LEERLAUF_MAX_MINUTEN)? * 60);
                i += 1;
            }
            "--leerlauf-sekunden" => {
                befehle.push(arg);
                leerlauf = Some(zahl(arg, naechstes, LEERLAUF_MAX_MINUTEN * 60)?);
                i += 1;
            }
            "--version" | "-V" => befehle.push("--version"),
            "--hilfe" | "--help" | "-h" => befehle.push("--hilfe"),
            "--sofort" => sofort = true,
            "--countdown" => {
                countdown = Some(zahl(arg, naechstes, COUNTDOWN_MAX)?);
                i += 1;
            }
            _ => return fehler(format!("unbekannte Option: {arg} (siehe --hilfe)")),
        }
        i += 1;
    }
    let befehl = match befehle.as_slice() {
        [] => return fehler("kein Befehl angegeben (siehe --hilfe)"),
        [einer] => *einer,
        mehrere => {
            return fehler(format!(
                "nur ein Befehl je Aufruf, nicht {}",
                mehrere.join(" und ")
            ));
        }
    };
    if sofort && befehl != "--sperren" {
        return fehler("--sofort gibt es nur zusammen mit --sperren");
    }
    if countdown.is_some()
        && !matches!(
            befehl,
            "--dialog" | "--abmelden" | "--neustart" | "--herunterfahren"
        )
    {
        return fehler(
            "--countdown gilt nur für --dialog, --abmelden, --neustart und --herunterfahren",
        );
    }
    Ok(match befehl {
        "--version" => Befehl::Version,
        "--hilfe" => Befehl::Hilfe,
        "--dialog" => Befehl::Dialog { countdown },
        "--leerlauf" | "--leerlauf-sekunden" => Befehl::Leerlauf {
            sekunden: leerlauf.expect("Leerlaufzeit geparst"),
        },
        "--sperren" if sofort => Befehl::SperrenSofort,
        "--sperren" => Befehl::Rueckfrage {
            aktion: Aktion::Sperren,
            countdown: None,
        },
        _ => Befehl::Rueckfrage {
            aktion: aktion.expect("Aktion gesetzt"),
            countdown,
        },
    })
}

/// Ob `args` den Countdown ausdrücklich festlegen (`--countdown`); dann
/// gilt die Befehlszeile, nicht `sitzung.ini`.
pub fn countdown_ausdruecklich<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter().any(|a| a.as_ref() == "--countdown")
}

/// Für `Countdown=an` in `sitzung.ini`: Abmelden, Neu starten und
/// Herunterfahren zählen [`COUNTDOWN_STANDARD`] Sekunden herunter; andere
/// Befehle bleiben, wie sie sind.
pub fn mit_countdown(befehl: Befehl) -> Befehl {
    match befehl {
        Befehl::Rueckfrage {
            aktion,
            countdown: None,
        } if aktion != Aktion::Sperren => Befehl::Rueckfrage {
            aktion,
            countdown: Some(COUNTDOWN_STANDARD),
        },
        andere => andere,
    }
}

pub const HILFE: &str = "\
Aufruf: querencia-sitzung BEFEHL [OPTIONEN]

Befehle (genau einer):
  --dialog            Abmelden, Bildschirm sperren, Neu starten, Herunterfahren
  --abmelden          Abmelden mit Rückfrage
  --sperren           Bildschirm sperren mit Rückfrage
  --sperren --sofort  Bildschirm sofort sperren (Tastenkürzel, Leerlauf)
  --neustart          Neu starten mit Rückfrage
  --herunterfahren    Herunterfahren mit Rückfrage; der Dialog bietet auch
                      Neu starten an (Abbrechen, Neu starten, Herunterfahren)
  --leerlauf MIN      Leerlaufdienst: sperrt nach MIN Minuten ohne Eingabe
  --leerlauf-sekunden SEK   dasselbe in Sekunden (für Prüfungen)
                      Gesperrt schaltet der Leerlaufdienst die Bildschirme
                      nach 2 min ohne Eingabe ab; ~/.config/querencia/
                      sitzung.ini mit [Sperre] BildschirmAus=SEK|Nmin, 0 = nie.
  --version, --hilfe

Optionen:
  --countdown SEK     Countdown in Sekunden (1 bis 3600); ohne ihn wartet der
                      Dialog, bis jemand wählt. Er zählt nur, solange der
                      Dialog aktiv ist, und bricht nach 60 s ohne Fokus ab.
                      Für alle Rückfragen einschalten: ~/.config/querencia/
                      sitzung.ini mit [Dialog] Countdown=an (60 s).

Umgebung:
  QS_TROCKENLAUF=1    Abmelden, Neu starten und Herunterfahren werden nur
                      protokolliert, nicht ausgeführt (für Prüfungen)
  QS_PAM_DIENST       PAM-Dienst des Sperrbildschirms (Standard:
                      querencia-sitzung, falls in /etc/pam.d, sonst login)
";

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Befehl, Fehler> {
        parse(args.iter().copied())
    }

    #[test]
    fn dialog_ohne_countdown_standard() {
        assert_eq!(p(&["--dialog"]), Ok(Befehl::Dialog { countdown: None }));
        assert_eq!(
            p(&["--dialog", "--countdown", "30"]),
            Ok(Befehl::Dialog {
                countdown: Some(30)
            })
        );
        assert!(p(&["--dialog", "--kein-countdown"]).is_err());
    }

    #[test]
    fn rueckfragen_warten_ohne_countdown() {
        for (arg, aktion) in [
            ("--abmelden", Aktion::Abmelden),
            ("--neustart", Aktion::Neustart),
            ("--herunterfahren", Aktion::Herunterfahren),
        ] {
            assert_eq!(
                p(&[arg]),
                Ok(Befehl::Rueckfrage {
                    aktion,
                    countdown: None
                }),
                "{arg}"
            );
            assert_eq!(
                p(&["--countdown", "5", arg]),
                Ok(Befehl::Rueckfrage {
                    aktion,
                    countdown: Some(5)
                })
            );
            assert!(p(&[arg, "--kein-countdown"]).is_err(), "{arg}");
        }
    }

    #[test]
    fn sperren_mit_und_ohne_rueckfrage() {
        assert_eq!(
            p(&["--sperren"]),
            Ok(Befehl::Rueckfrage {
                aktion: Aktion::Sperren,
                countdown: None
            })
        );
        assert_eq!(p(&["--sperren", "--sofort"]), Ok(Befehl::SperrenSofort));
        assert_eq!(p(&["--sofort", "--sperren"]), Ok(Befehl::SperrenSofort));
        assert!(p(&["--sperren", "--countdown", "10"]).is_err());
    }

    #[test]
    fn sofort_nur_fuer_sperren() {
        for arg in ["--abmelden", "--neustart", "--herunterfahren", "--dialog"] {
            let e = p(&[arg, "--sofort"]).unwrap_err();
            assert!(e.0.contains("--sofort"), "{arg}: {e}");
        }
    }

    #[test]
    fn leerlauf_in_minuten_oder_sekunden() {
        assert_eq!(
            p(&["--leerlauf", "10"]),
            Ok(Befehl::Leerlauf { sekunden: 600 })
        );
        assert_eq!(
            p(&["--leerlauf-sekunden", "5"]),
            Ok(Befehl::Leerlauf { sekunden: 5 })
        );
        for bad in [
            vec!["--leerlauf"],
            vec!["--leerlauf", "0"],
            vec!["--leerlauf", "x"],
            vec!["--leerlauf", "-3"],
            vec!["--leerlauf", "1441"],
        ] {
            assert!(p(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn genau_ein_befehl() {
        assert!(p(&[]).is_err());
        let e = p(&["--dialog", "--abmelden"]).unwrap_err();
        assert!(e.0.contains("nur ein Befehl"), "{e}");
        assert!(p(&["--unsinn"]).is_err());
        assert!(p(&["--countdown", "abc", "--abmelden"]).is_err());
        assert!(p(&["--countdown", "0", "--abmelden"]).is_err());
        assert!(p(&["--countdown", "3601", "--abmelden"]).is_err());
        assert!(p(&["--countdown", "3600", "--abmelden"]).is_ok());
    }

    #[test]
    fn version_und_hilfe() {
        assert_eq!(p(&["--version"]), Ok(Befehl::Version));
        assert_eq!(p(&["-V"]), Ok(Befehl::Version));
        assert_eq!(p(&["--hilfe"]), Ok(Befehl::Hilfe));
        assert_eq!(p(&["--help"]), Ok(Befehl::Hilfe));
        assert_eq!(p(&["-h"]), Ok(Befehl::Hilfe));
        assert!(HILFE.contains("--sperren --sofort"));
        assert!(HILFE.contains("Abbrechen, Neu starten, Herunterfahren"));
    }

    #[test]
    fn countdown_ausdruecklich_und_einschalten() {
        assert!(countdown_ausdruecklich(&["--abmelden", "--countdown", "3"]));
        assert!(!countdown_ausdruecklich(&["--abmelden"]));
        assert_eq!(
            mit_countdown(p(&["--abmelden"]).unwrap()),
            Befehl::Rueckfrage {
                aktion: Aktion::Abmelden,
                countdown: Some(COUNTDOWN_STANDARD)
            }
        );
        // ausdrücklicher Wert bleibt, Sperren und Gesamtdialog zählen nicht
        assert_eq!(
            mit_countdown(p(&["--neustart", "--countdown", "9"]).unwrap()),
            Befehl::Rueckfrage {
                aktion: Aktion::Neustart,
                countdown: Some(9)
            }
        );
        assert_eq!(
            mit_countdown(p(&["--sperren"]).unwrap()),
            Befehl::Rueckfrage {
                aktion: Aktion::Sperren,
                countdown: None
            }
        );
        assert_eq!(
            mit_countdown(p(&["--dialog"]).unwrap()),
            Befehl::Dialog { countdown: None }
        );
        assert_eq!(mit_countdown(Befehl::Version), Befehl::Version);
    }

    #[test]
    fn aktionsnamen() {
        assert_eq!(Aktion::Abmelden.name(), "Abmelden");
        assert_eq!(Aktion::Sperren.name(), "Bildschirm sperren");
        assert_eq!(Aktion::Neustart.name(), "Neu starten");
        assert_eq!(Aktion::Herunterfahren.name(), "Herunterfahren");
    }
}
