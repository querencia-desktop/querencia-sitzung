//! Befehlszeile von `querencia-sitzung`: genau ein Befehl je Aufruf.
//!
//! ```text
//! querencia-sitzung --dialog [--countdown SEK | --kein-countdown]
//! querencia-sitzung --abmelden | --neustart | --herunterfahren
//!                   [--countdown SEK | --kein-countdown]
//! querencia-sitzung --sperren [--sofort]
//! querencia-sitzung --leerlauf MINUTEN | --leerlauf-sekunden SEK
//! querencia-sitzung --version | --hilfe
//! ```
//!
//! Die Rückfragen mit nur einer Aktion zählen wie GNOME 2 sechzig Sekunden
//! herunter; `--dialog` zählt nur auf Wunsch (dann bis zum Abmelden).

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
    let mut countdown: Option<Option<u32>> = None;
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
                countdown = Some(Some(zahl(arg, naechstes, COUNTDOWN_MAX)?));
                i += 1;
            }
            "--kein-countdown" => countdown = Some(None),
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
            "--countdown und --kein-countdown gelten nur für --dialog, --abmelden, --neustart und --herunterfahren",
        );
    }
    Ok(match befehl {
        "--version" => Befehl::Version,
        "--hilfe" => Befehl::Hilfe,
        "--dialog" => Befehl::Dialog {
            countdown: countdown.unwrap_or(None),
        },
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
            countdown: countdown.unwrap_or(Some(COUNTDOWN_STANDARD)),
        },
    })
}

/// Ob `args` den Countdown ausdrücklich festlegen (`--countdown` oder
/// `--kein-countdown`); dann gilt die Befehlszeile, nicht `sitzung.ini`.
pub fn countdown_ausdruecklich<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter()
        .any(|a| matches!(a.as_ref(), "--countdown" | "--kein-countdown"))
}

/// `befehl` ohne Countdown.
pub fn ohne_countdown(befehl: Befehl) -> Befehl {
    match befehl {
        Befehl::Dialog { .. } => Befehl::Dialog { countdown: None },
        Befehl::Rueckfrage { aktion, .. } => Befehl::Rueckfrage {
            aktion,
            countdown: None,
        },
        andere => andere,
    }
}

pub const HILFE: &str = "\
Aufruf: querencia-sitzung BEFEHL [OPTIONEN]

Befehle (genau einer):
  --dialog            Abmelden, Bildschirm sperren, Neu starten, Herunterfahren
  --abmelden          Abmelden mit Rückfrage (Countdown 60 s)
  --sperren           Bildschirm sperren mit Rückfrage
  --sperren --sofort  Bildschirm sofort sperren (Tastenkürzel, Leerlauf)
  --neustart          Neu starten mit Rückfrage (Countdown 60 s)
  --herunterfahren    Herunterfahren mit Rückfrage (Countdown 60 s)
  --leerlauf MIN      Leerlaufdienst: sperrt nach MIN Minuten ohne Eingabe
  --leerlauf-sekunden SEK   dasselbe in Sekunden (für Prüfungen)
  --version, --hilfe

Optionen:
  --countdown SEK     Countdown in Sekunden (1 bis 3600)
  --kein-countdown    kein automatisches Auslösen
  Der Countdown zählt nur, solange der Dialog aktiv ist, und bricht nach
  60 s ohne Fokus ab. Abschalten für alle Dialoge: ~/.config/querencia/
  sitzung.ini mit [Dialog] Countdown=aus (die Befehlszeile geht vor).

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
        assert_eq!(
            p(&["--dialog", "--kein-countdown"]),
            Ok(Befehl::Dialog { countdown: None })
        );
    }

    #[test]
    fn rueckfragen_zaehlen_sechzig_sekunden() {
        for (arg, aktion) in [
            ("--abmelden", Aktion::Abmelden),
            ("--neustart", Aktion::Neustart),
            ("--herunterfahren", Aktion::Herunterfahren),
        ] {
            assert_eq!(
                p(&[arg]),
                Ok(Befehl::Rueckfrage {
                    aktion,
                    countdown: Some(60)
                }),
                "{arg}"
            );
            assert_eq!(
                p(&[arg, "--kein-countdown"]),
                Ok(Befehl::Rueckfrage {
                    aktion,
                    countdown: None
                })
            );
            assert_eq!(
                p(&["--countdown", "5", arg]),
                Ok(Befehl::Rueckfrage {
                    aktion,
                    countdown: Some(5)
                })
            );
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
    }

    #[test]
    fn countdown_ausdruecklich_und_abschalten() {
        assert!(countdown_ausdruecklich(&["--abmelden", "--countdown", "3"]));
        assert!(countdown_ausdruecklich(&["--kein-countdown", "--neustart"]));
        assert!(!countdown_ausdruecklich(&["--abmelden"]));
        assert_eq!(
            ohne_countdown(p(&["--abmelden"]).unwrap()),
            Befehl::Rueckfrage {
                aktion: Aktion::Abmelden,
                countdown: None
            }
        );
        assert_eq!(
            ohne_countdown(p(&["--dialog", "--countdown", "9"]).unwrap()),
            Befehl::Dialog { countdown: None }
        );
        assert_eq!(ohne_countdown(Befehl::Version), Befehl::Version);
    }

    #[test]
    fn aktionsnamen() {
        assert_eq!(Aktion::Abmelden.name(), "Abmelden");
        assert_eq!(Aktion::Sperren.name(), "Bildschirm sperren");
        assert_eq!(Aktion::Neustart.name(), "Neu starten");
        assert_eq!(Aktion::Herunterfahren.name(), "Herunterfahren");
    }
}
