//! Passwortprüfung über PAM (Linux-PAM, `libpam.so.0`).
//!
//! Auf der VM fehlen die PAM-Entwicklungsdateien (kein `libpam0g-dev`,
//! kein `security/pam_appl.h`), die Crates `pam` und `pam-client` binden
//! aber zur Bauzeit gegen `libpam.so`. Darum lädt dieses Modul die
//! Bibliothek zur Laufzeit mit `dlopen` und spricht die vier Funktionen
//! `pam_start`, `pam_authenticate`, `pam_acct_mgmt` und `pam_end` direkt an.
//! Das ist der einzige Ort mit `unsafe` neben dem Shared-Memory-Puffer
//! des Sperrbildschirms.
//!
//! Dienstname: `QS_PAM_DIENST`, sonst `querencia-sitzung`, falls
//! `/etc/pam.d/querencia-sitzung` existiert (Vorschlag in `integration/`),
//! sonst `login` (vorhanden, enthält `common-auth`; mit `pam_faildelay`
//! von drei Sekunden nach einem Fehlversuch).

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;

const PAM_SUCCESS: c_int = 0;
const PAM_AUTH_ERR: c_int = 7;
const PAM_USER_UNKNOWN: c_int = 10;
const PAM_MAXTRIES: c_int = 11;
const PAM_NEW_AUTHTOK_REQD: c_int = 12;
const PAM_ACCT_EXPIRED: c_int = 13;
const PAM_PERM_DENIED: c_int = 6;
const PAM_CONV_ERR: c_int = 19;

const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;
const PAM_ERROR_MSG: c_int = 3;
const PAM_TEXT_INFO: c_int = 4;

const PAM_TTY: c_int = 3;

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

type ConvFn = unsafe extern "C" fn(
    c_int,
    *mut *const PamMessage,
    *mut *mut PamResponse,
    *mut c_void,
) -> c_int;

#[repr(C)]
struct PamConv {
    conv: ConvFn,
    appdata_ptr: *mut c_void,
}

type PamStart =
    unsafe extern "C" fn(*const c_char, *const c_char, *const PamConv, *mut *mut c_void) -> c_int;
type PamSimple = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type PamSetItem = unsafe extern "C" fn(*mut c_void, c_int, *const c_void) -> c_int;
type PamStrerror = unsafe extern "C" fn(*mut c_void, c_int) -> *const c_char;

/// Daten für die Konversation: das Passwort und die Meldungen der Module.
struct Gespraech {
    passwort: CString,
    meldungen: Vec<String>,
}

/// Antwortet auf Passwortfragen mit dem Passwort, sammelt Meldungen.
/// PAM gibt die Antworten mit `free` frei, darum `calloc`/`strdup`.
unsafe extern "C" fn konversation(
    anzahl: c_int,
    nachrichten: *mut *const PamMessage,
    antworten: *mut *mut PamResponse,
    daten: *mut c_void,
) -> c_int {
    if anzahl <= 0 || nachrichten.is_null() || antworten.is_null() || daten.is_null() {
        return PAM_CONV_ERR;
    }
    // SAFETY: PAM übergibt `anzahl` gültige Zeiger; `daten` ist unser
    // `Gespraech`, das `pruefen` bis nach `pam_end` am Leben hält.
    unsafe {
        let gespraech = &mut *(daten as *mut Gespraech);
        let n = anzahl as usize;
        let antwort = libc::calloc(n, std::mem::size_of::<PamResponse>()) as *mut PamResponse;
        if antwort.is_null() {
            return PAM_CONV_ERR;
        }
        for i in 0..n {
            // Linux-PAM: `nachrichten` ist ein Feld von Zeigern auf Nachrichten.
            let nachricht = *nachrichten.add(i);
            if nachricht.is_null() {
                libc::free(antwort as *mut c_void);
                return PAM_CONV_ERR;
            }
            let text = if (*nachricht).msg.is_null() {
                String::new()
            } else {
                CStr::from_ptr((*nachricht).msg)
                    .to_string_lossy()
                    .into_owned()
            };
            match (*nachricht).msg_style {
                PAM_PROMPT_ECHO_OFF | PAM_PROMPT_ECHO_ON => {
                    let kopie = libc::strdup(gespraech.passwort.as_ptr());
                    if kopie.is_null() {
                        libc::free(antwort as *mut c_void);
                        return PAM_CONV_ERR;
                    }
                    (*antwort.add(i)).resp = kopie;
                }
                PAM_ERROR_MSG | PAM_TEXT_INFO => gespraech.meldungen.push(text),
                _ => {
                    libc::free(antwort as *mut c_void);
                    return PAM_CONV_ERR;
                }
            }
        }
        *antworten = antwort;
    }
    PAM_SUCCESS
}

/// Ergebnis einer Passwortprüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ergebnis {
    /// Passwort richtig, Konto in Ordnung.
    Richtig,
    /// Passwort falsch (oder Konto gesperrt): Meldung für den Bildschirm.
    Falsch(String),
    /// PAM selbst nicht benutzbar (Bibliothek, Dienst, Konversation).
    Fehler(String),
}

/// Deutsche Meldung zu einem PAM-Rückgabewert.
pub fn meldung(code: c_int, strerror: Option<&str>) -> String {
    match code {
        PAM_AUTH_ERR => "Falsches Passwort".to_string(),
        PAM_MAXTRIES => "Zu viele Fehlversuche, bitte kurz warten".to_string(),
        PAM_USER_UNKNOWN => "Benutzer unbekannt".to_string(),
        PAM_NEW_AUTHTOK_REQD => "Das Passwort muss geändert werden (Konsole)".to_string(),
        PAM_ACCT_EXPIRED => "Das Konto ist abgelaufen".to_string(),
        PAM_PERM_DENIED => "Zugriff verweigert".to_string(),
        _ => match strerror {
            Some(s) if !s.is_empty() => format!("PAM: {s} ({code})"),
            _ => format!("PAM-Fehler {code}"),
        },
    }
}

/// Nicht `Richtig`, nicht `Fehler`: ein abgelehnter Versuch.
fn abgelehnt(code: c_int) -> bool {
    matches!(
        code,
        PAM_AUTH_ERR
            | PAM_MAXTRIES
            | PAM_USER_UNKNOWN
            | PAM_NEW_AUTHTOK_REQD
            | PAM_ACCT_EXPIRED
            | PAM_PERM_DENIED
    )
}

/// PAM-Dienst nach `QS_PAM_DIENST`, sonst nach den vorhandenen Dateien.
pub fn dienstname(umgebung: Option<&str>, eigener_dienst_vorhanden: bool) -> String {
    match umgebung {
        Some(d) if !d.is_empty() && !d.contains('/') => d.to_string(),
        _ if eigener_dienst_vorhanden => "querencia-sitzung".to_string(),
        _ => "login".to_string(),
    }
}

/// [`dienstname`] für dieses System.
pub fn dienst() -> String {
    dienstname(
        std::env::var("QS_PAM_DIENST").ok().as_deref(),
        Path::new("/etc/pam.d/querencia-sitzung").is_file(),
    )
}

struct Bibliothek {
    handle: *mut c_void,
    start: PamStart,
    authenticate: PamSimple,
    acct_mgmt: PamSimple,
    end: PamSimple,
    set_item: PamSetItem,
    strerror: PamStrerror,
}

impl Bibliothek {
    fn laden() -> Result<Bibliothek, String> {
        let name = c"libpam.so.0";
        // SAFETY: dlopen/dlsym mit gültigen C-Strings; die Symbole haben in
        // Linux-PAM die hier deklarierten Signaturen.
        unsafe {
            let handle = libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if handle.is_null() {
                return Err("libpam.so.0 nicht ladbar".to_string());
            }
            let symbol = |s: &CStr| -> Result<*mut c_void, String> {
                let p = libc::dlsym(handle, s.as_ptr());
                if p.is_null() {
                    Err(format!("{} fehlt in libpam.so.0", s.to_string_lossy()))
                } else {
                    Ok(p)
                }
            };
            Ok(Bibliothek {
                handle,
                start: std::mem::transmute::<*mut c_void, PamStart>(symbol(c"pam_start")?),
                authenticate: std::mem::transmute::<*mut c_void, PamSimple>(symbol(
                    c"pam_authenticate",
                )?),
                acct_mgmt: std::mem::transmute::<*mut c_void, PamSimple>(symbol(c"pam_acct_mgmt")?),
                end: std::mem::transmute::<*mut c_void, PamSimple>(symbol(c"pam_end")?),
                set_item: std::mem::transmute::<*mut c_void, PamSetItem>(symbol(c"pam_set_item")?),
                strerror: std::mem::transmute::<*mut c_void, PamStrerror>(symbol(c"pam_strerror")?),
            })
        }
    }

    fn text(&self, pamh: *mut c_void, code: c_int) -> Option<String> {
        // SAFETY: pam_strerror liefert einen statischen String oder NULL.
        unsafe {
            let p = (self.strerror)(pamh, code);
            (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
        }
    }
}

impl Drop for Bibliothek {
    fn drop(&mut self) {
        // SAFETY: handle stammt aus dlopen; danach wird kein Symbol mehr benutzt.
        unsafe {
            libc::dlclose(self.handle);
        }
    }
}

/// Prüft `passwort` für `benutzer` beim PAM-Dienst `dienst`
/// (`pam_authenticate`, dann `pam_acct_mgmt`). Blockiert, bei falschem
/// Passwort je nach Dienst einige Sekunden; der Sperrbildschirm ruft es in
/// einem eigenen Thread.
pub fn pruefen(dienst: &str, benutzer: &str, passwort: &str) -> Ergebnis {
    let lib = match Bibliothek::laden() {
        Ok(l) => l,
        Err(e) => return Ergebnis::Fehler(e),
    };
    let (Ok(dienst_c), Ok(benutzer_c), Ok(passwort_c)) = (
        CString::new(dienst),
        CString::new(benutzer),
        CString::new(passwort),
    ) else {
        return Ergebnis::Fehler("Eingabe enthält ein Nullzeichen".to_string());
    };
    let mut gespraech = Box::new(Gespraech {
        passwort: passwort_c,
        meldungen: Vec::new(),
    });
    let conv = PamConv {
        conv: konversation,
        appdata_ptr: &mut *gespraech as *mut Gespraech as *mut c_void,
    };
    let mut pamh: *mut c_void = std::ptr::null_mut();
    // SAFETY: Aufrufe nach der Linux-PAM-Schnittstelle; `conv` und
    // `gespraech` leben bis nach pam_end.
    let ergebnis = unsafe {
        let rc = (lib.start)(dienst_c.as_ptr(), benutzer_c.as_ptr(), &conv, &mut pamh);
        if rc != PAM_SUCCESS || pamh.is_null() {
            return Ergebnis::Fehler(format!(
                "pam_start({dienst}) fehlgeschlagen: {}",
                meldung(rc, lib.text(std::ptr::null_mut(), rc).as_deref())
            ));
        }
        // Wie Sperrprogramme üblich: ein Terminalname, damit Module mit
        // TTY-Bezug nicht mit leerem Wert scheitern.
        let tty = c"querencia-sitzung";
        (lib.set_item)(pamh, PAM_TTY, tty.as_ptr() as *const c_void);
        let mut rc = (lib.authenticate)(pamh, 0);
        if rc == PAM_SUCCESS {
            rc = (lib.acct_mgmt)(pamh, 0);
        }
        let text = lib.text(pamh, rc);
        (lib.end)(pamh, rc);
        if rc == PAM_SUCCESS {
            Ergebnis::Richtig
        } else if abgelehnt(rc) {
            Ergebnis::Falsch(meldung(rc, text.as_deref()))
        } else {
            Ergebnis::Fehler(meldung(rc, text.as_deref()))
        }
    };
    if !gespraech.meldungen.is_empty() {
        eprintln!(
            "querencia-sitzung: PAM-Meldungen: {}",
            gespraech.meldungen.join(" | ")
        );
    }
    ergebnis
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dienstwahl() {
        assert_eq!(dienstname(None, false), "login");
        assert_eq!(dienstname(None, true), "querencia-sitzung");
        assert_eq!(dienstname(Some(""), true), "querencia-sitzung");
        assert_eq!(dienstname(Some("greetd"), true), "greetd");
        // Pfade sind keine Dienstnamen
        assert_eq!(dienstname(Some("/etc/pam.d/login"), false), "login");
    }

    #[test]
    fn meldungen_deutsch() {
        assert_eq!(meldung(PAM_AUTH_ERR, None), "Falsches Passwort");
        assert!(meldung(PAM_MAXTRIES, None).contains("Fehlversuche"));
        assert_eq!(meldung(99, Some("Unknown")), "PAM: Unknown (99)");
        assert_eq!(meldung(99, None), "PAM-Fehler 99");
        assert!(abgelehnt(PAM_AUTH_ERR));
        assert!(!abgelehnt(PAM_SUCCESS));
        assert!(!abgelehnt(PAM_CONV_ERR));
    }

    /// Echte PAM-Prüfung mit einem sicher falschen Passwort: beweist, dass
    /// Bibliothek, Dienst und Konversation funktionieren (der Dienst
    /// `login` wartet dabei drei Sekunden). Kein echtes Passwort im Test.
    #[test]
    fn falsches_passwort_wird_abgelehnt() {
        let benutzer = std::env::var("USER").unwrap_or_else(|_| "nobody".to_string());
        match pruefen("login", &benutzer, "querencia-sicher-falsch-\u{1F512}") {
            Ergebnis::Falsch(m) => assert!(!m.is_empty()),
            andere => panic!("erwartet Falsch(..), bekommen {andere:?}"),
        }
    }

    #[test]
    fn unbekannter_dienst_ist_fehler_oder_ablehnung() {
        // Ein Dienst ohne Datei in /etc/pam.d läuft über "other" (pam_deny):
        // niemals Richtig.
        let e = pruefen("querencia-gibt-es-nicht", "nobody", "x");
        assert_ne!(e, Ergebnis::Richtig);
    }

    #[test]
    fn nullzeichen_abgewiesen() {
        assert!(matches!(pruefen("login", "a\0b", "x"), Ergebnis::Fehler(_)));
    }
}
