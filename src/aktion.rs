//! Ausführen der Sitzungsaktionen.
//!
//! * Abmelden: zuerst Panel, Arbeitsfläche, Benachrichtigungen,
//!   polkit-Agent, Leerlaufdienst und Applet-Helfer dieser Sitzung beenden
//!   (`aufraeumen.rs`, QD-062), dann xdg-desktop-portal(-gtk) stoppen, dann
//!   wie das Panel (QREL-04d, `session.rs`)
//!   `<labwc> --exit` mit dem labwc der laufenden Sitzung aus
//!   `/proc/$LABWC_PID/exe`; `querencia-session` räumt danach auf.
//! * Neu starten, Herunterfahren: systemd-logind über D-Bus
//!   (`org.freedesktop.login1.Manager.Reboot`/`PowerOff`, interactive =
//!   true, damit polkit nachfragen kann), vorher `CanReboot`/`CanPowerOff`.
//! * Sperren: der Sperrbildschirm (`sperre.rs`) ersetzt diesen Prozess.
//!
//! Mit `QS_TROCKENLAUF=1` wird Abmelden, Neu starten und Herunterfahren nur
//! protokolliert (Prüfungen; auf der Entwicklungs-VM nie auslösen).

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use crate::cli::Aktion;
use crate::dialog::titel;
use crate::i18n::{gettext, gettext_f};

/// `QS_TROCKENLAUF=1`: nur protokollieren.
pub fn trockenlauf() -> bool {
    std::env::var("QS_TROCKENLAUF").is_ok_and(|v| v == "1")
}

/// Protokollzeile auf stderr.
pub fn protokoll(text: &str) {
    eprintln!("querencia-sitzung: {text}");
}

/// Das labwc, dessen `--exit` diese Sitzung beendet: das Programm des
/// Prozesses `labwc_pid` (`$LABWC_PID`) laut `exe`, wenn es ein labwc ist.
/// Übernommen aus dem Panel (QREL-04d, `session.rs`), damit beide Wege
/// dasselbe tun.
pub fn labwc_exit_command(
    labwc_pid: Option<&str>,
    exe: impl Fn(u32) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let pid: u32 = labwc_pid?.parse().ok().filter(|pid| *pid > 0)?;
    let path = exe(pid)?;
    let name = path.file_name()?.to_str()?;
    (name == "labwc" || name.starts_with("labwc-")).then_some(path)
}

/// [`labwc_exit_command`] für diesen Prozess.
pub fn labwc_der_sitzung() -> Option<PathBuf> {
    labwc_exit_command(std::env::var("LABWC_PID").ok().as_deref(), |pid| {
        std::fs::read_link(format!("/proc/{pid}/exe")).ok()
    })
}

/// Abmelden: erst die Sitzungsprogramme beenden (`aufraeumen.rs`), dann
/// `<labwc> --exit` (erbt `LABWC_PID`).
pub fn abmelden() -> Result<(), String> {
    let labwc = labwc_der_sitzung()
        .ok_or_else(|| gettext("Cannot log out: no labwc of this session (LABWC_PID)"))?;
    if let Some(pid) = std::env::var("LABWC_PID")
        .ok()
        .and_then(|p| p.parse::<u32>().ok())
    {
        crate::aufraeumen::sitzung_beenden(pid);
    }
    crate::aufraeumen::portale_stoppen();
    if trockenlauf() {
        protokoll(&format!("TROCKENLAUF: {} --exit", labwc.display()));
        return Ok(());
    }
    let status = Command::new(&labwc)
        .arg("--exit")
        .stdin(Stdio::null())
        .status()
        .map_err(|e| {
            gettext_f(
                "{program} --exit failed: {error}",
                &[
                    ("program", &labwc.display().to_string()),
                    ("error", &e.to_string()),
                ],
            )
        })?;
    protokoll(&format!("{} --exit beendet mit {status}", labwc.display()));
    if status.success() {
        Ok(())
    } else {
        Err(gettext_f(
            "{program} --exit reported {status}",
            &[
                ("program", &labwc.display().to_string()),
                ("status", &status.to_string()),
            ],
        ))
    }
}

/// Deutet die Antwort von `CanReboot`/`CanPowerOff`: `yes` und `challenge`
/// (polkit fragt nach) erlauben den Aufruf, `no` und `na` nicht. `was` ist
/// der übersetzte Name der Aktion („Restart“).
pub fn erlaubnis(antwort: &str, was: &str) -> Result<(), String> {
    match antwort {
        "yes" | "challenge" => Ok(()),
        // TRANSLATORS: {action} ist „Restart“ oder „Shut Down“.
        "no" => Err(gettext_f(
            "{action} is not allowed for this user (logind: no). \
             Please run it at the console or as an administrator.",
            &[("action", was)],
        )),
        "na" => Err(gettext_f(
            "{action} is not possible on this system (logind: na).",
            &[("action", was)],
        )),
        andere => Err(gettext_f(
            "{action}: unexpected answer from logind: “{answer}”",
            &[("action", was), ("answer", andere)],
        )),
    }
}

/// D-Bus-Methode am logind-Manager, Ergebnis als Variant.
fn logind(
    methode: &str,
    parameter: Option<&glib::Variant>,
    timeout_ms: i32,
) -> Result<glib::Variant, String> {
    let bus = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>).map_err(|e| {
        gettext_f(
            "System D-Bus not reachable: {error}",
            &[("error", &e.to_string())],
        )
    })?;
    bus.call_sync(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        methode,
        parameter,
        None,
        gio::DBusCallFlags::NONE,
        timeout_ms,
        None::<&gio::Cancellable>,
    )
    .map_err(|e| format!("logind {methode}: {e}"))
}

/// Antwort von `CanReboot`/`CanPowerOff` als Text.
fn logind_kann(methode: &str) -> Result<String, String> {
    let antwort = logind(methode, None, 5000)?;
    antwort
        .get::<(String,)>()
        .map(|(s,)| s)
        .ok_or_else(|| format!("logind {methode}: unerwartete Antwort {antwort}"))
}

/// Neu starten oder Herunterfahren über logind; `was` ist der Name für
/// Meldungen, `kann`/`tue` die Methoden.
fn logind_aktion(was: &str, kann: &str, tue: &str) -> Result<(), String> {
    let antwort = logind_kann(kann)?;
    protokoll(&format!("logind {kann}: {antwort}"));
    erlaubnis(&antwort, was)?;
    if trockenlauf() {
        protokoll(&format!(
            "TROCKENLAUF: logind {tue}(interactive=true) nicht aufgerufen"
        ));
        return Ok(());
    }
    // Lange Frist: polkit darf den Benutzer fragen.
    logind(tue, Some(&(true,).to_variant()), 120_000).map(|_| ())
}

/// Pfad dieses Programms (für Sperre und Leerlaufdienst).
pub fn eigenes_programm() -> PathBuf {
    std::fs::read_link("/proc/self/exe")
        .or_else(|_| std::env::current_exe())
        .unwrap_or_else(|_| PathBuf::from("querencia-sitzung"))
}

/// Ersetzt diesen Prozess durch `querencia-sitzung --sperren --sofort`.
/// Kehrt nur bei Fehler zurück.
pub fn sperren_ausfuehren(programm: &Path) -> String {
    let err = Command::new(programm)
        .args(["--sperren", "--sofort"])
        .exec();
    gettext_f(
        "Cannot start the lock screen ({program}): {error}",
        &[
            ("program", &programm.display().to_string()),
            ("error", &err.to_string()),
        ],
    )
}

/// Führt die bestätigte Aktion aus; Sperren kehrt bei Erfolg nicht zurück.
pub fn ausfuehren(aktion: Aktion) -> Result<(), String> {
    protokoll(&format!("Aktion: {}", aktion.name()));
    match aktion {
        Aktion::Abmelden => abmelden(),
        Aktion::Neustart => logind_aktion(&titel(aktion), "CanReboot", "Reboot"),
        Aktion::Herunterfahren => logind_aktion(&titel(aktion), "CanPowerOff", "PowerOff"),
        Aktion::Sperren => Err(sperren_ausfuehren(&eigenes_programm())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe_of(pid: u32) -> Option<PathBuf> {
        match pid {
            1816 => Some(PathBuf::from("/home/user/opt/labwc-0.9.8/bin/labwc")),
            42 => Some(PathBuf::from("/usr/bin/labwc")),
            7 => Some(PathBuf::from("/usr/bin/bash")),
            _ => None,
        }
    }

    #[test]
    fn labwc_der_sitzung_wird_gefunden() {
        assert_eq!(
            labwc_exit_command(Some("1816"), exe_of).as_deref(),
            Some(Path::new("/home/user/opt/labwc-0.9.8/bin/labwc"))
        );
        assert_eq!(
            labwc_exit_command(Some("42"), exe_of).as_deref(),
            Some(Path::new("/usr/bin/labwc"))
        );
    }

    #[test]
    fn kein_labwc_kein_abmelden() {
        assert_eq!(labwc_exit_command(None, exe_of), None);
        for bad in ["", "abc", "-1", "0", "18 16", "1816\n"] {
            assert_eq!(labwc_exit_command(Some(bad), exe_of), None, "{bad:?}");
        }
        assert_eq!(labwc_exit_command(Some("99"), exe_of), None);
        assert_eq!(labwc_exit_command(Some("7"), exe_of), None);
        let any_labwc = |_| Some(PathBuf::from("/usr/bin/labwc"));
        assert_eq!(labwc_exit_command(Some("0"), any_labwc), None);
    }

    #[test]
    fn logind_antworten() {
        // Ohne Katalog englisch (msgid), deutsch siehe po/de.po.
        assert_eq!(erlaubnis("yes", "Restart"), Ok(()));
        assert_eq!(erlaubnis("challenge", "Restart"), Ok(()));
        let e = erlaubnis("no", "Shut Down").unwrap_err();
        assert!(
            e.starts_with("Shut Down is not allowed for this user"),
            "{e}"
        );
        let e = erlaubnis("na", "Restart").unwrap_err();
        assert!(e.contains("not possible"), "{e}");
        assert!(erlaubnis("maybe", "Restart").is_err());
    }

    #[test]
    fn trockenlauf_nur_bei_eins() {
        // Nur Lesen der Umgebung; die Tests laufen ohne QS_TROCKENLAUF oder
        // mit QS_TROCKENLAUF=1, beides zulässig.
        let v = std::env::var("QS_TROCKENLAUF").ok();
        assert_eq!(trockenlauf(), v.as_deref() == Some("1"));
    }

    /// Nur lesen: `CanReboot` antwortet mit einem der vier Werte.
    #[test]
    fn logind_can_reboot_antwortet() {
        match logind_kann("CanReboot") {
            Ok(a) => assert!(
                ["yes", "no", "na", "challenge"].contains(&a.as_str()),
                "{a}"
            ),
            Err(e) => eprintln!("kein logind erreichbar (zulässig im Test): {e}"),
        }
    }
}
