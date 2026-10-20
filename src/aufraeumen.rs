//! Abmelden beendet zuerst die eigenen Programme der Sitzung (QD-062).
//!
//! Bisher meldete `<labwc> --exit` allein ab; Panel, Arbeitsfläche,
//! Benachrichtigungen, polkit-Agent und Leerlaufdienst endeten erst, wenn
//! sie den Verlust der Wayland-Verbindung bemerkten (das Panel am
//! 20.10.2026 sechs Sekunden später). Wer sich gleich wieder anmeldete,
//! traf noch auf sie: Der Sitzungsstarter verweigert den Start, solange ein
//! Panel läuft, und der alte Benachrichtigungsdienst hält
//! `org.freedesktop.Notifications`. Jetzt schickt Abmelden ihnen SIGTERM,
//! wartet [`FRIST_TERM`] und erzwingt den Rest mit SIGKILL; erst danach
//! folgen [`portale_stoppen`] und `labwc --exit`.
//!
//! Wer zur Sitzung gehört: Prozesse dieses Benutzers, deren Programm eines
//! der Sitzungsprogramme ist ([`ist_sitzungsprogramm`]) und die
//! - in ihrer Umgebung `LABWC_PID` dieser Sitzung tragen (labwc gibt sie
//!   über den Autostart an alles weiter, was er startet), oder
//! - ohne `LABWC_PID` in einer PID-Datei des Sitzungsstarters unter
//!   `~/.local/state/querencia` stehen („PID STARTZEIT“, z. B. ein von Hand
//!   getauschtes Panel).
//!
//! Isolierte Prüfsitzungen haben einen eigenen labwc und damit eine andere
//! `LABWC_PID`; sie bleiben unberührt. Die Prozessgruppe des
//! Sitzungsstarters hilft hier nicht: labwc startet den Autostart in einer
//! eigenen Sitzung (setsid), die Gruppe enthält nur Starter und labwc.
//! Programme, die der Benutzer gestartet hat (Files, Terminal …), bleiben
//! labwc überlassen.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::aktion::{protokoll, trockenlauf};

/// Frist zwischen SIGTERM und SIGKILL.
pub const FRIST_TERM: Duration = Duration::from_secs(2);
/// Frist nach SIGKILL.
pub const FRIST_KILL: Duration = Duration::from_secs(1);

/// Programme der Sitzung, die Abmelden beendet (Dateiname der Binary).
pub const PROGRAMME: &[&str] = &[
    "querencia-panel",
    "querencia-desktop",
    "querencia-benachrichtigungen",
    "querencia-polkit-agent",
    "querencia-lautstaerke",
    "querencia-netzwerk",
];

/// Ob ein Prozess mit Programmdatei `name` und Argumenten `args` (ohne
/// Programmnamen) zur Sitzung gehört. `querencia-sitzung` nur als
/// Leerlaufdienst: Dialoge und den Sperrbildschirm (auch diesen Prozess)
/// nie.
pub fn ist_sitzungsprogramm(name: &str, args: &[String]) -> bool {
    PROGRAMME.contains(&name)
        || (name == "querencia-sitzung"
            && args
                .iter()
                .any(|a| a == "--leerlauf" || a == "--leerlauf-sekunden"))
}

/// Ein gefundener Prozess der Sitzung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prozess {
    pub pid: u32,
    /// Startzeit (Feld 22 von `/proc/PID/stat`), erkennt wiederverwendete PIDs.
    pub start: u64,
    pub name: String,
}

/// Feld 22 (Startzeit) und Feld 3 (Zustand) aus dem Inhalt von
/// `/proc/PID/stat`; der Programmname in Klammern darf Leerzeichen und
/// Klammern enthalten.
pub fn stat_felder(stat: &str) -> Option<(char, u64)> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let felder: Vec<&str> = rest.split_whitespace().collect();
    // rest beginnt mit Feld 3 (Zustand); Feld 22 ist dort Index 19.
    let zustand = felder.first()?.chars().next()?;
    let start = felder.get(19)?.parse().ok()?;
    Some((zustand, start))
}

/// Reale UID aus dem Inhalt von `/proc/PID/status`.
pub fn status_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|z| z.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|uid| uid.parse().ok())
}

/// „PID STARTZEIT“ aus einer PID-Datei des Sitzungsstarters.
pub fn pid_datensatz(text: &str) -> Option<(u32, u64)> {
    let mut teile = text.split_whitespace();
    let pid = teile.next()?.parse().ok()?;
    let start = teile.next()?.parse().ok()?;
    teile.next().is_none().then_some((pid, start))
}

/// Alle Datensätze der `*.pid`-Dateien in `ordner` (andere Formate wie
/// `labwc.pid` fallen heraus).
pub fn pid_dateien(ordner: &Path) -> Vec<(u32, u64)> {
    let Ok(eintraege) = std::fs::read_dir(ordner) else {
        return Vec::new();
    };
    eintraege
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".pid"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| pid_datensatz(&t))
        .collect()
}

/// Zustandsordner des Sitzungsstarters (`QUERENCIA_STATE_DIR`, sonst
/// `$XDG_STATE_HOME/querencia`, sonst `~/.local/state/querencia`).
pub fn zustandsordner() -> PathBuf {
    let absolut = |v: std::ffi::OsString| {
        let p = PathBuf::from(v);
        p.is_absolute().then_some(p)
    };
    if let Some(p) = std::env::var_os("QUERENCIA_STATE_DIR").and_then(absolut) {
        return p;
    }
    let state = std::env::var_os("XDG_STATE_HOME")
        .and_then(absolut)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")
        });
    state.join("querencia")
}

/// Was [`lesen`] über einen Prozess erfährt.
struct Gelesen {
    uid: u32,
    name: String,
    args: Vec<String>,
    /// `LABWC_PID` der Umgebung.
    labwc: Option<String>,
    zustand: char,
    start: u64,
}

/// Liest einen Prozess unter `proc_root/<pid>`.
fn lesen(proc_root: &Path, pid: u32) -> Option<Gelesen> {
    let dir = proc_root.join(pid.to_string());
    let uid = status_uid(&std::fs::read_to_string(dir.join("status")).ok()?)?;
    let exe = std::fs::read_link(dir.join("exe")).ok()?;
    let name = exe.file_name()?.to_string_lossy().into_owned();
    // ersetzte Binary: „querencia-panel (deleted)“
    let name = name.strip_suffix(" (deleted)").unwrap_or(&name).to_string();
    let args: Vec<String> = std::fs::read(dir.join("cmdline"))
        .ok()?
        .split(|b| *b == 0)
        .skip(1)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect();
    let labwc = std::fs::read(dir.join("environ")).ok().and_then(|env| {
        env.split(|b| *b == 0)
            .find_map(|e| e.strip_prefix(b"LABWC_PID="))
            .map(|v| String::from_utf8_lossy(v).into_owned())
    });
    let (zustand, start) = stat_felder(&std::fs::read_to_string(dir.join("stat")).ok()?)?;
    Some(Gelesen {
        uid,
        name,
        args,
        labwc,
        zustand,
        start,
    })
}

/// Die Prozesse der Sitzung von labwc `labwc_pid` unter `proc_root`
/// (sonst `/proc`), ohne `eigene_pid`; `uid` ist der Benutzer,
/// `datensaetze` die PID-Dateien des Starters.
pub fn finden(
    proc_root: &Path,
    uid: u32,
    labwc_pid: u32,
    eigene_pid: u32,
    datensaetze: &[(u32, u64)],
) -> Vec<Prozess> {
    let Ok(eintraege) = std::fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let labwc_text = labwc_pid.to_string();
    let mut gefunden: Vec<Prozess> = eintraege
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| *pid != eigene_pid && *pid != labwc_pid)
        .filter_map(|pid| {
            let g = lesen(proc_root, pid)?;
            let zugehoerig = match g.labwc.as_deref() {
                Some(l) => l == labwc_text,
                None => datensaetze.contains(&(pid, g.start)),
            };
            (g.uid == uid
                && g.zustand != 'Z'
                && zugehoerig
                && ist_sitzungsprogramm(&g.name, &g.args))
            .then_some(Prozess {
                pid,
                start: g.start,
                name: g.name,
            })
        })
        .collect();
    gefunden.sort_by_key(|p| p.pid);
    gefunden
}

/// Ob `p` noch läuft (gleiche Startzeit, kein Zombie).
fn laeuft(p: &Prozess) -> bool {
    std::fs::read_to_string(format!("/proc/{}/stat", p.pid))
        .ok()
        .and_then(|s| stat_felder(&s))
        .is_some_and(|(zustand, start)| start == p.start && zustand != 'Z')
}

fn signal(p: &Prozess, sig: i32) {
    if laeuft(p) {
        // SAFETY: kill(2) mit einer geprüften PID dieses Benutzers; die
        // Startzeit wurde unmittelbar davor verglichen.
        unsafe { libc::kill(p.pid as libc::pid_t, sig) };
    }
}

/// Wartet bis `frist`, bis keiner aus `liste` mehr läuft; liefert die
/// übrigen.
fn warten(liste: &[Prozess], frist: Duration) -> Vec<Prozess> {
    let ende = Instant::now() + frist;
    loop {
        let rest: Vec<Prozess> = liste.iter().filter(|p| laeuft(p)).cloned().collect();
        if rest.is_empty() || Instant::now() >= ende {
            return rest;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn liste_text(liste: &[Prozess]) -> String {
    liste
        .iter()
        .map(|p| format!("{} ({})", p.name, p.pid))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Beendet die Sitzungsprozesse von labwc `labwc_pid` (SIGTERM, Frist,
/// SIGKILL) und protokolliert jeden Schritt. Im Trockenlauf nur die Liste.
pub fn sitzung_beenden(labwc_pid: u32) {
    // SAFETY: getuid(2) hat keine Vorbedingungen.
    let uid = unsafe { libc::getuid() };
    let datensaetze = pid_dateien(&zustandsordner());
    let liste = finden(
        Path::new("/proc"),
        uid,
        labwc_pid,
        std::process::id(),
        &datensaetze,
    );
    if liste.is_empty() {
        protokoll("Abmelden: keine Sitzungsprogramme mehr zu beenden");
        return;
    }
    if trockenlauf() {
        protokoll(&format!(
            "TROCKENLAUF: würde beenden: {}",
            liste_text(&liste)
        ));
        return;
    }
    protokoll(&format!("Abmelden: SIGTERM an {}", liste_text(&liste)));
    for p in &liste {
        signal(p, libc::SIGTERM);
    }
    let rest = warten(&liste, FRIST_TERM);
    if rest.is_empty() {
        protokoll("Abmelden: alle Sitzungsprogramme beendet");
        return;
    }
    protokoll(&format!(
        "Abmelden: nach {} s noch da, SIGKILL an {}",
        FRIST_TERM.as_secs(),
        liste_text(&rest)
    ));
    for p in &rest {
        signal(p, libc::SIGKILL);
    }
    let rest = warten(&rest, FRIST_KILL);
    if rest.is_empty() {
        protokoll("Abmelden: alle Sitzungsprogramme beendet");
    } else {
        protokoll(&format!("Abmelden: nicht beendet: {}", liste_text(&rest)));
    }
}

/// Portal-Dienste des User-Managers, die am Wayland-Display der Sitzung
/// hängen.
pub const PORTALE: &[&str] = &[
    "xdg-desktop-portal-gtk.service",
    "xdg-desktop-portal.service",
];

/// Die `systemctl --user`-Aufrufe für [`portale_stoppen`]: `reset-failed`,
/// `stop`, `reset-failed`.
pub fn portal_befehle() -> Vec<Vec<String>> {
    ["reset-failed", "stop", "reset-failed"]
        .iter()
        .map(|befehl| {
            let mut args = vec!["--user".to_string(), befehl.to_string()];
            args.extend(PORTALE.iter().map(|u| u.to_string()));
            args
        })
        .collect()
}

/// Stoppt xdg-desktop-portal und sein GTK-Backend vor `labwc --exit`
/// (QD-062). Endet labwc zuerst, stirbt das GTK-Backend, xdg-desktop-portal
/// startet es ohne Display neu, die Unit bleibt `failed`, und die nächste
/// Anmeldung wartet auf ein stehendes Portal. Gestoppt startet D-Bus beide
/// beim nächsten Bedarf frisch. Der Sitzungsstarter stoppt sie nach dem
/// Ende von labwc noch einmal (falls ein Programm sie bis dahin wieder
/// aktiviert hat). gvfs und document-portal bleiben. Im Trockenlauf nur
/// eine Logzeile.
pub fn portale_stoppen() {
    let befehle = portal_befehle();
    if trockenlauf() {
        protokoll(&format!(
            "TROCKENLAUF: würde Portale stoppen ({})",
            PORTALE.join(", ")
        ));
        return;
    }
    let mut ergebnis = Vec::new();
    for args in &befehle {
        let status = std::process::Command::new("systemctl")
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        ergebnis.push(format!(
            "{} {}",
            args[1],
            match status {
                Ok(s) if s.success() => "ok".to_string(),
                Ok(s) => s.to_string(),
                Err(e) => e.to_string(),
            }
        ));
    }
    protokoll(&format!(
        "Abmelden: Portale vor labwc --exit gestoppt ({}): {}",
        PORTALE.join(", "),
        ergebnis.join(", ")
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn sitzungsprogramme_und_nur_der_leerlaufdienst() {
        for name in PROGRAMME {
            assert!(ist_sitzungsprogramm(name, &[]), "{name}");
        }
        assert!(ist_sitzungsprogramm(
            "querencia-sitzung",
            &s(&["--leerlauf", "10"])
        ));
        assert!(ist_sitzungsprogramm(
            "querencia-sitzung",
            &s(&["--leerlauf-sekunden", "5"])
        ));
        for args in [vec![], s(&["--abmelden"]), s(&["--sperren", "--sofort"])] {
            assert!(
                !ist_sitzungsprogramm("querencia-sitzung", &args),
                "{args:?}"
            );
        }
        for name in [
            "strata",
            "querencia-terminal",
            "labwc",
            "bash",
            "querencia-panelx",
        ] {
            assert!(!ist_sitzungsprogramm(name, &[]), "{name}");
        }
    }

    #[test]
    fn portale_reset_stop_reset() {
        let b = portal_befehle();
        let namen: Vec<&str> = b.iter().map(|a| a[1].as_str()).collect();
        assert_eq!(namen, ["reset-failed", "stop", "reset-failed"]);
        for a in &b {
            assert_eq!(a[0], "--user");
            assert_eq!(&a[2..], PORTALE);
        }
        assert!(
            !PORTALE
                .iter()
                .any(|u| u.contains("gvfs") || u.contains("document"))
        );
    }

    #[test]
    fn stat_mit_klammern_im_namen() {
        let stat = "2299 (querencia (x) panel) S 1 2289 2289 0 -1 4194560 1 2 3 4 5 6 7 8 20 0 9 0 1495 1000 2";
        assert_eq!(stat_felder(stat), Some(('S', 1495)));
        assert_eq!(stat_felder("kaputt"), None);
        assert_eq!(stat_felder("1 (a) Z"), None);
    }

    #[test]
    fn status_und_datensatz() {
        assert_eq!(
            status_uid("Name:\tx\nUid:\t1001\t1001\t1001\t1001\n"),
            Some(1001)
        );
        assert_eq!(status_uid("Name:\tx\n"), None);
        assert_eq!(pid_datensatz("2299 1495\n"), Some((2299, 1495)));
        assert_eq!(pid_datensatz("q2 51c2 2257 1495"), None);
        assert_eq!(pid_datensatz("2299"), None);
        assert_eq!(pid_datensatz("2299 1495 7"), None);
    }

    /// Ein Prozess im nachgebauten /proc (ein Argument je Angabe in /proc).
    #[allow(clippy::too_many_arguments)]
    fn prozess(
        root: &Path,
        pid: u32,
        uid: u32,
        exe: &str,
        args: &[&str],
        labwc: Option<&str>,
        zustand: char,
        start: u64,
    ) {
        let d = root.join(pid.to_string());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("status"),
            format!("Name:\tx\nUid:\t{uid}\t{uid}\t{uid}\t{uid}\n"),
        )
        .unwrap();
        symlink(exe, d.join("exe")).unwrap();
        let mut cmd = exe.as_bytes().to_vec();
        for a in args {
            cmd.push(0);
            cmd.extend_from_slice(a.as_bytes());
        }
        cmd.push(0);
        std::fs::write(d.join("cmdline"), cmd).unwrap();
        let env = match labwc {
            Some(l) => format!("HOME=/h\0LABWC_PID={l}\0"),
            None => "HOME=/h\0".to_string(),
        };
        std::fs::write(d.join("environ"), env).unwrap();
        std::fs::write(
            d.join("stat"),
            format!("{pid} (x) {zustand} 1 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 {start} 1 1"),
        )
        .unwrap();
    }

    #[test]
    fn findet_nur_die_eigene_sitzung() {
        let tmp = std::env::temp_dir().join(format!("qs-aufraeumen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let r = tmp.as_path();
        let opt = "/home/u/opt/querencia";
        prozess(
            r,
            100,
            1001,
            "/home/u/opt/labwc/bin/labwc",
            &[],
            None,
            'S',
            10,
        );
        prozess(
            r,
            101,
            1001,
            &format!("{opt}/panel/a/querencia-panel"),
            &[],
            Some("100"),
            'S',
            11,
        );
        prozess(
            r,
            102,
            1001,
            &format!("{opt}/desktop/b/querencia-desktop (deleted)"),
            &[],
            Some("100"),
            'S',
            12,
        );
        prozess(
            r,
            103,
            1001,
            &format!("{opt}/sitzung/c/querencia-sitzung"),
            &["--leerlauf", "10"],
            Some("100"),
            'S',
            13,
        );
        // dieser Prozess (Dialog) und ein anderer Dialog
        prozess(
            r,
            104,
            1001,
            &format!("{opt}/sitzung/c/querencia-sitzung"),
            &["--abmelden"],
            Some("100"),
            'S',
            14,
        );
        prozess(
            r,
            105,
            1001,
            &format!("{opt}/sitzung/c/querencia-sitzung"),
            &["--dialog"],
            Some("100"),
            'S',
            15,
        );
        // isolierte Prüfsitzung (anderer labwc), fremder Benutzer, Zombie
        prozess(
            r,
            106,
            1001,
            &format!("{opt}/panel/a/querencia-panel"),
            &[],
            Some("900"),
            'S',
            16,
        );
        prozess(
            r,
            107,
            1002,
            &format!("{opt}/panel/a/querencia-panel"),
            &[],
            Some("100"),
            'S',
            17,
        );
        prozess(
            r,
            108,
            1001,
            &format!("{opt}/x/querencia-benachrichtigungen"),
            &[],
            Some("100"),
            'Z',
            18,
        );
        // Benutzerprogramm der Sitzung bleibt
        prozess(r, 109, 1001, "/usr/bin/foot", &[], Some("100"), 'S', 19);
        // ohne LABWC_PID: nur mit passendem Datensatz
        prozess(
            r,
            110,
            1001,
            &format!("{opt}/panel/a/querencia-panel"),
            &[],
            None,
            'S',
            20,
        );
        prozess(
            r,
            111,
            1001,
            &format!("{opt}/desktop/b/querencia-desktop"),
            &[],
            None,
            'S',
            21,
        );
        let gefunden = finden(r, 1001, 100, 104, &[(110, 20), (111, 99)]);
        let pids: Vec<u32> = gefunden.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![101, 102, 103, 110]);
        assert_eq!(gefunden[1].name, "querencia-desktop");
        assert_eq!(gefunden[0].start, 11);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn pid_dateien_lesen() {
        let tmp = std::env::temp_dir().join(format!("qs-piddateien-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("panel.2212.pid"), "2299 1495\n").unwrap();
        std::fs::write(tmp.join("labwc.pid"), "q2 b 2257 1495\n").unwrap();
        std::fs::write(tmp.join("desktop.2212.pid"), "2297 1494\n").unwrap();
        std::fs::write(tmp.join("panel.log"), "2299 1495\n").unwrap();
        let mut d = pid_dateien(&tmp);
        d.sort();
        assert_eq!(d, vec![(2297, 1494), (2299, 1495)]);
        std::fs::remove_dir_all(&tmp).unwrap();
        assert!(pid_dateien(&tmp).is_empty());
    }
}
