//! Dunkelmodus für fremde GTK-3-Programme und die labwc-Fensterrahmen
//! (0.1.5, QD-103, QD-088 Punkt 6), geführt vom Leerlaufdienst.
//!
//! Die Querencia-Programme (GTK 4) folgen dem Farbschema über das
//! Settings-Portal selbst. Zwei Teile der Sitzung tun das nicht:
//! - **GTK 3** (3.24 unter Wayland, isoliert gemessen,
//!   `nachweis/015-gtk3-messung`) kennt das Farbschema des Portals nicht
//!   (Fall m2). Thema und Schrift nimmt es live aus den gsettings, die
//!   Dunkelfassung aber nur aus `gtk-application-prefer-dark-theme` in
//!   `$XDG_CONFIG_HOME/gtk-3.0/settings.ini`, und das nur beim Start (m3,
//!   m4, m10). Ein Abschnitt `[Settings]`, der am Dateiende noch einmal
//!   beginnt, geht früheren Einträgen vor (m6). Laufende GTK-3-Programme
//!   wechseln deshalb erst beim nächsten Start.
//! - **labwc** zeichnet Titelleisten und Rahmen nach seinem Thema. Es liest
//!   danach `<Konfigordner>/themerc-override`; dessen Werte gehen dem Thema
//!   vor (`src/theme.c`, `theme_init`). `labwc --reconfigure` ist ein
//!   SIGHUP an `LABWC_PID`.
//!
//! Kriterium ist `dunkel_aktiv` wie in Panel und Einstellungen: Farbschema
//! `prefer-dark` **und** ein GTK-Thema mit `gtk-4.0/gtk-dark.css`. Den
//! GTK-3-Block gibt es zusätzlich nur, wenn das Thema `gtk-3.0/gtk-dark.css`
//! hat. Beide Dateien bekommen einen gekennzeichneten Block zwischen
//! [`ANFANG`] und [`ENDE`]; fremde Zeilen bleiben byte-gleich. Bei „hell“
//! fällt der Block weg, eine Datei, die nur noch aus ihm bestand, wird
//! entfernt. Verweise (Symlinks) und Dateien mit beschädigtem Block bleiben
//! unberührt.

use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

use gtk::gio;
use gtk::prelude::*;

use crate::aktion::{labwc_exit_command, protokoll};

pub const ANFANG: &str =
    "# >>> querencia-sitzung: Dunkelmodus (von der Sitzung verwaltet, nicht von Hand ändern)";
pub const ENDE: &str = "# <<< querencia-sitzung";

/// Block für `gtk-3.0/settings.ini`. `[Settings]` beginnt neu, weil davor
/// ein anderer Abschnitt stehen kann (Messung m6).
pub const GTK3_BLOCK: &str = "[Settings]\ngtk-application-prefer-dark-theme=true\n";

/// Block für `themerc-override`. **Platzhalter**, bis das labwc-Dunkelthema
/// von Thema 0.3 (Worker thema-022) vorliegt: Farben aus der Dunkelfassung
/// von Querencia-Klassisch (GTK, Thema 0.2.1): Grund `#3c3b37`,
/// Seitenleiste/Menü `#353430`, Schrift `#dfdbd2`, Umriss `#23221f` und
/// Grau `#bdb7b1` wie die Statussymbole im Panel (QD-088 Nr. 4), Fehlerrot
/// `#ed8b85` (QD-088 Nr. 1). Kontrast nach WCAG 2: aktiver Titel `#eeeeec`
/// auf `#3c3b37` 9,65:1, inaktiver `#bdb7b1` auf `#353430` 6,27:1,
/// Fensterwechsler und Menü `#dfdbd2` auf `#353430` 9,02:1.
/// Die Titelknöpfe sind Bilder des Themas (PNG) und bleiben, wie sie sind.
pub const THEMERC_DUNKEL: &str = "\
window.active.border.color: #23221f
window.inactive.border.color: #2b2a27
window.active.title.bg: Gradient Vertical
window.active.title.bg.color: #3c3b37
window.active.title.bg.colorTo: #353430
window.inactive.title.bg: Solid
window.inactive.title.bg.color: #353430
window.active.label.text.color: #eeeeec
window.inactive.label.text.color: #bdb7b1
window.active.button.unpressed.image.color: #eeeeec
window.inactive.button.unpressed.image.color: #bdb7b1
window.active.button.close.unpressed.image.color: #ed8b85
window.inactive.button.close.unpressed.image.color: #c46e6e
osd.bg.color: #353430
osd.border.color: #23221f
osd.label.text.color: #dfdbd2
menu.items.bg.color: #353430
menu.items.text.color: #dfdbd2
menu.items.active.bg.color: #4a4944
menu.items.active.text.color: #eeeeec
menu.border.color: #23221f
menu.separator.color: #4a4944
menu.title.bg.color: #3c3b37
menu.title.text.color: #eeeeec
";

const SCHEMA: &str = "org.gnome.desktop.interface";
/// GTKs eingebautes Thema hat eine Dunkelfassung; „Default“ nennt GTK es,
/// wenn keines gesetzt ist.
const EINGEBAUT: [&str; 2] = ["Default", "Adwaita"];

/// `text` ohne verwaltete Blöcke, dahinter `block` (falls angegeben) mit
/// [`ANFANG`] und [`ENDE`]. Fremde Zeilen bleiben byte-gleich. Ein Block
/// ohne Ende ist ein Fehler: dann bleibt die Datei, wie sie ist.
pub fn block_einsetzen(text: &str, block: Option<&str>) -> Result<String, String> {
    let mut aus = String::with_capacity(text.len() + 256);
    let mut drin = false;
    for zeile in text.split_inclusive('\n') {
        let z = zeile.trim_end();
        if !drin && z == ANFANG {
            drin = true;
        } else if drin && z == ENDE {
            drin = false;
        } else if drin && z == ANFANG {
            return Err("verwalteter Block beginnt zweimal".into());
        } else if !drin {
            aus.push_str(zeile);
        }
    }
    if drin {
        return Err("verwalteter Block ohne Ende".into());
    }
    if let Some(block) = block {
        if !aus.is_empty() && !aus.ends_with('\n') {
            aus.push('\n');
        }
        aus.push_str(ANFANG);
        aus.push('\n');
        aus.push_str(block);
        if !block.ends_with('\n') {
            aus.push('\n');
        }
        aus.push_str(ENDE);
        aus.push('\n');
    }
    Ok(aus)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Abgleich {
    Unveraendert,
    Geschrieben,
    Entfernt,
}

/// Bringt den verwalteten Block in `pfad` auf `block` (oder entfernt ihn).
/// Neue Fassungen entstehen daneben und ersetzen die Datei per `rename`;
/// die Rechte einer bestehenden Datei bleiben.
pub fn datei_abgleichen(pfad: &Path, block: Option<&str>) -> Result<Abgleich, String> {
    let ort = pfad.display();
    let (text, rechte) = match std::fs::symlink_metadata(pfad) {
        Ok(m) if m.file_type().is_symlink() => {
            return Err(format!("{ort} ist ein Verweis, bleibt unberührt"));
        }
        Ok(m) if !m.is_file() => return Err(format!("{ort} ist keine Datei")),
        Ok(m) => (
            std::fs::read_to_string(pfad).map_err(|e| format!("{ort}: {e}"))?,
            Some(m.permissions()),
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => (String::new(), None),
        Err(e) => return Err(format!("{ort}: {e}")),
    };
    let neu = block_einsetzen(&text, block).map_err(|e| format!("{ort}: {e}, bleibt unberührt"))?;
    if neu == text {
        return Ok(Abgleich::Unveraendert);
    }
    if neu.is_empty() {
        std::fs::remove_file(pfad).map_err(|e| format!("{ort}: {e}"))?;
        return Ok(Abgleich::Entfernt);
    }
    if let Some(ordner) = pfad.parent() {
        std::fs::create_dir_all(ordner).map_err(|e| format!("{}: {e}", ordner.display()))?;
    }
    let mut tmp = pfad.as_os_str().to_owned();
    tmp.push(".querencia-sitzung-neu");
    let tmp = PathBuf::from(tmp);
    let schreiben = || -> std::io::Result<()> {
        std::fs::write(&tmp, &neu)?;
        let rechte = rechte
            .clone()
            .unwrap_or_else(|| std::fs::Permissions::from_mode(0o644));
        std::fs::set_permissions(&tmp, rechte)?;
        std::fs::rename(&tmp, pfad)
    };
    schreiben().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{ort}: {e}")
    })?;
    Ok(Abgleich::Geschrieben)
}

/// Was die Sitzung vorfindet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lage {
    pub schema_dunkel: bool,
    pub thema_gtk4_dunkel: bool,
    pub thema_gtk3_dunkel: bool,
}

/// Was die Sitzung daraus macht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Soll {
    pub dunkel_aktiv: bool,
    pub gtk3: bool,
    pub labwc: bool,
}

pub fn soll(lage: Lage) -> Soll {
    let dunkel_aktiv = lage.schema_dunkel && lage.thema_gtk4_dunkel;
    Soll {
        dunkel_aktiv,
        gtk3: dunkel_aktiv && lage.thema_gtk3_dunkel,
        labwc: dunkel_aktiv,
    }
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".config"))
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// Ordner, in denen GTK Themen sucht, in seiner Reihenfolge.
pub fn themenordner() -> Vec<PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".local/share"));
    let mut ordner = vec![data_home.join("themes"), home().join(".themes")];
    let dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    ordner.extend(
        dirs.split(':')
            .filter(|d| d.starts_with('/'))
            .map(|d| Path::new(d).join("themes")),
    );
    ordner
}

/// Hat das Thema `name` in `gtk` (`gtk-4.0` oder `gtk-3.0`) eine
/// Dunkelfassung? Wie in GTK zählt der erste Ordner, in dem das Thema
/// `<gtk>/gtk.css` hat.
pub fn thema_mit_dunkel(ordner: &[PathBuf], name: &str, gtk: &str) -> bool {
    ordner
        .iter()
        .map(|d| d.join(name).join(gtk))
        .find(|d| d.join("gtk.css").is_file())
        .map_or(EINGEBAUT.contains(&name), |d| {
            d.join("gtk-dark.css").is_file()
        })
}

/// Konfigordner aus der Befehlszeile von labwc (`-C`, `--config-dir`).
pub fn labwc_konfig_aus_argv(argv: &[String]) -> Option<PathBuf> {
    let mut it = argv.iter().skip(1);
    let mut gefunden = None;
    while let Some(a) = it.next() {
        if a == "-C" || a == "--config-dir" {
            gefunden = it.next().map(PathBuf::from);
        } else if let Some(o) = a.strip_prefix("--config-dir=") {
            gefunden = Some(PathBuf::from(o));
        } else if let Some(o) = a.strip_prefix("-C").filter(|o| !o.is_empty()) {
            gefunden = Some(PathBuf::from(o));
        }
    }
    gefunden
}

/// Der Ordner, in dem labwc `themerc-override` liest: `-C` der Befehlszeile,
/// sonst `$XDG_CONFIG_HOME/labwc`.
pub fn labwc_konfigordner(argv: &[String], config_home: &Path) -> PathBuf {
    labwc_konfig_aus_argv(argv).unwrap_or_else(|| config_home.join("labwc"))
}

fn kanonisch(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Die Sitzung schreibt nur in Benutzerordner unter `$XDG_CONFIG_HOME`:
/// läuft labwc mit der Vorgabe im Runtime-Repo oder einem Systemordner,
/// bleibt der Rahmen hell.
pub fn ordner_erlaubt(ordner: &Path, config_home: &Path) -> bool {
    ordner.is_absolute()
        && !ordner.components().any(|c| c == Component::ParentDir)
        && kanonisch(ordner).starts_with(kanonisch(config_home))
}

/// labwc dieser Sitzung: PID aus `LABWC_PID` (nur wenn `/proc/<pid>/exe`
/// ein labwc ist) und sein Konfigordner.
fn labwc_der_sitzung() -> Result<(i32, PathBuf), String> {
    let pid = std::env::var("LABWC_PID").map_err(|_| "LABWC_PID fehlt".to_string())?;
    labwc_exit_command(Some(&pid), |p| {
        std::fs::read_link(format!("/proc/{p}/exe")).ok()
    })
    .ok_or_else(|| format!("LABWC_PID {pid} ist kein labwc"))?;
    let roh = std::fs::read(format!("/proc/{pid}/cmdline"))
        .map_err(|e| format!("Befehlszeile von labwc {pid}: {e}"))?;
    let argv: Vec<String> = roh
        .split(|b| *b == 0)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect();
    let pid: i32 = pid.parse().map_err(|_| format!("LABWC_PID {pid}"))?;
    Ok((pid, labwc_konfigordner(&argv, &config_home())))
}

/// `labwc --reconfigure`: SIGHUP an labwc.
fn labwc_neu_laden(pid: i32) {
    // SAFETY: kill(2) mit einer geprüften PID (exe = labwc) und SIGHUP.
    if unsafe { libc::kill(pid, libc::SIGHUP) } == 0 {
        protokoll(&format!(
            "Dunkelmodus: labwc {pid} lädt das Thema neu (SIGHUP)"
        ));
    } else {
        protokoll(&format!(
            "Dunkelmodus: SIGHUP an labwc {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }
}

fn melden(was: &str, pfad: &Path, r: Result<Abgleich, String>) -> bool {
    match r {
        Ok(Abgleich::Unveraendert) => false,
        Ok(Abgleich::Geschrieben) => {
            protokoll(&format!(
                "Dunkelmodus: {was} {} geschrieben",
                pfad.display()
            ));
            true
        }
        Ok(Abgleich::Entfernt) => {
            protokoll(&format!("Dunkelmodus: {was} {} entfernt", pfad.display()));
            true
        }
        Err(e) => {
            protokoll(&format!("Dunkelmodus: {was}: {e}"));
            false
        }
    }
}

fn anwenden(s: &gio::Settings, anlass: &str) {
    let schema = s.string("color-scheme");
    let thema = s.string("gtk-theme");
    let ordner = themenordner();
    let lage = Lage {
        schema_dunkel: schema == "prefer-dark",
        thema_gtk4_dunkel: thema_mit_dunkel(&ordner, &thema, "gtk-4.0"),
        thema_gtk3_dunkel: thema_mit_dunkel(&ordner, &thema, "gtk-3.0"),
    };
    let soll = soll(lage);
    protokoll(&format!(
        "Dunkelmodus ({anlass}): Farbschema {schema}, Thema {thema} (GTK 4 dunkel {}, GTK 3 dunkel {}) → dunkel_aktiv {}",
        lage.thema_gtk4_dunkel, lage.thema_gtk3_dunkel, soll.dunkel_aktiv
    ));
    let ini = config_home().join("gtk-3.0").join("settings.ini");
    melden(
        "GTK 3",
        &ini,
        datei_abgleichen(&ini, soll.gtk3.then_some(GTK3_BLOCK)),
    );
    match labwc_der_sitzung() {
        Ok((pid, ordner)) if ordner_erlaubt(&ordner, &config_home()) => {
            let datei = ordner.join("themerc-override");
            let r = datei_abgleichen(&datei, soll.labwc.then_some(THEMERC_DUNKEL));
            if melden("labwc-Rahmen", &datei, r) {
                labwc_neu_laden(pid);
            }
        }
        Ok((_, ordner)) => protokoll(&format!(
            "Dunkelmodus: labwc-Konfigordner {} liegt nicht unter {}, Rahmen bleiben unverändert",
            ordner.display(),
            config_home().display()
        )),
        Err(e) => protokoll(&format!("Dunkelmodus: labwc-Rahmen: {e}")),
    }
}

/// Gleicht beim Start ab und dann bei jedem Wechsel von Farbschema oder
/// Thema. Das Ergebnis muss leben, solange der Dienst läuft; `None` ohne
/// das Schema `org.gnome.desktop.interface` (dann geschieht nichts).
pub fn starten() -> Option<gio::Settings> {
    let quelle = gio::SettingsSchemaSource::default()?;
    let schema = quelle.lookup(SCHEMA, true)?;
    if !schema.has_key("color-scheme") || !schema.has_key("gtk-theme") {
        protokoll("Dunkelmodus: gsettings ohne color-scheme, fremde Programme bleiben hell");
        return None;
    }
    let s = gio::Settings::new(SCHEMA);
    anwenden(&s, "Start");
    s.connect_changed(None, |s, key| {
        if key == "color-scheme" || key == "gtk-theme" {
            anwenden(s, "Wechsel");
        }
    });
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qs-mitschalten-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn block_hinzu_und_weg_laesst_fremdes_byte_gleich() {
        for fremd in [
            "",
            "[Settings]\ngtk-font-name=DejaVu Sans 11\n",
            "[Settings]\ngtk-application-prefer-dark-theme=false\n\n[Andere]\nx=1\n",
        ] {
            let mit = block_einsetzen(fremd, Some(GTK3_BLOCK)).unwrap();
            assert!(mit.starts_with(fremd));
            assert!(mit.ends_with(&format!("{ANFANG}\n{GTK3_BLOCK}{ENDE}\n")));
            // zweimal setzen ändert nichts
            assert_eq!(block_einsetzen(&mit, Some(GTK3_BLOCK)).unwrap(), mit);
            assert_eq!(block_einsetzen(&mit, None).unwrap(), fremd);
        }
    }

    #[test]
    fn fremde_zeilen_nach_dem_block_bleiben() {
        let text = format!("a=1\n{ANFANG}\nalt\n{ENDE}\nb=2\n");
        assert_eq!(block_einsetzen(&text, None).unwrap(), "a=1\nb=2\n");
        assert_eq!(
            block_einsetzen(&text, Some("neu\n")).unwrap(),
            format!("a=1\nb=2\n{ANFANG}\nneu\n{ENDE}\n")
        );
    }

    #[test]
    fn ohne_zeilenende_wird_vor_dem_block_umgebrochen() {
        let mit = block_einsetzen("a=1", Some("x\n")).unwrap();
        assert_eq!(mit, format!("a=1\n{ANFANG}\nx\n{ENDE}\n"));
    }

    #[test]
    fn beschaedigter_block_wird_nicht_angefasst() {
        assert!(block_einsetzen(&format!("a\n{ANFANG}\nb\n"), None).is_err());
        assert!(block_einsetzen(&format!("{ANFANG}\n{ANFANG}\n{ENDE}\n"), None).is_err());
        // ein Ende ohne Anfang ist eine fremde Zeile
        assert_eq!(
            block_einsetzen(&format!("{ENDE}\n"), None).unwrap(),
            format!("{ENDE}\n")
        );
    }

    #[test]
    fn datei_anlegen_entfernen_und_rechte_behalten() {
        let d = tmp("datei");
        let p = d.join("gtk-3.0").join("settings.ini");
        assert_eq!(datei_abgleichen(&p, None).unwrap(), Abgleich::Unveraendert);
        assert!(!p.exists());
        assert_eq!(
            datei_abgleichen(&p, Some(GTK3_BLOCK)).unwrap(),
            Abgleich::Geschrieben
        );
        assert_eq!(
            datei_abgleichen(&p, Some(GTK3_BLOCK)).unwrap(),
            Abgleich::Unveraendert
        );
        assert_eq!(datei_abgleichen(&p, None).unwrap(), Abgleich::Entfernt);
        assert!(!p.exists());
        // fremde Datei mit eigenen Rechten
        std::fs::write(&p, "[Settings]\ngtk-font-name=X 9\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        datei_abgleichen(&p, Some(GTK3_BLOCK)).unwrap();
        let m = std::fs::metadata(&p).unwrap();
        assert_eq!(m.permissions().mode() & 0o777, 0o600);
        datei_abgleichen(&p, None).unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "[Settings]\ngtk-font-name=X 9\n"
        );
        assert!(
            !d.join("gtk-3.0/settings.ini.querencia-sitzung-neu")
                .exists()
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn verweis_bleibt_unberuehrt() {
        let d = tmp("verweis");
        let ziel = d.join("ziel.ini");
        std::fs::write(&ziel, "[Settings]\n").unwrap();
        let p = d.join("settings.ini");
        std::os::unix::fs::symlink(&ziel, &p).unwrap();
        assert!(datei_abgleichen(&p, Some(GTK3_BLOCK)).is_err());
        assert_eq!(std::fs::read_to_string(&ziel).unwrap(), "[Settings]\n");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn dunkel_aktiv_wie_panel_und_einstellungen() {
        let l = |s, g4, g3| Lage {
            schema_dunkel: s,
            thema_gtk4_dunkel: g4,
            thema_gtk3_dunkel: g3,
        };
        let hell = Soll {
            dunkel_aktiv: false,
            gtk3: false,
            labwc: false,
        };
        assert_eq!(soll(l(false, true, true)), hell);
        // Thema ohne Dunkelfassung (Querencia-Klassisch 0.1): alles hell
        assert_eq!(soll(l(true, false, true)), hell);
        assert_eq!(
            soll(l(true, true, true)),
            Soll {
                dunkel_aktiv: true,
                gtk3: true,
                labwc: true
            }
        );
        // GTK 3 nur mit gtk-3.0/gtk-dark.css
        assert_eq!(
            soll(l(true, true, false)),
            Soll {
                dunkel_aktiv: true,
                gtk3: false,
                labwc: true
            }
        );
    }

    #[test]
    fn thema_dunkelfassung_im_ersten_ordner() {
        let d = tmp("themen");
        let a = d.join("a");
        let b = d.join("b");
        for (o, dunkel) in [(&a, false), (&b, true)] {
            for gtk in ["gtk-3.0", "gtk-4.0"] {
                let t = o.join("T").join(gtk);
                std::fs::create_dir_all(&t).unwrap();
                std::fs::write(t.join("gtk.css"), "").unwrap();
                if dunkel {
                    std::fs::write(t.join("gtk-dark.css"), "").unwrap();
                }
            }
        }
        // a (ohne Dunkel) kommt zuerst und zählt
        assert!(!thema_mit_dunkel(&[a.clone(), b.clone()], "T", "gtk-4.0"));
        assert!(thema_mit_dunkel(&[b.clone(), a.clone()], "T", "gtk-3.0"));
        assert!(!thema_mit_dunkel(std::slice::from_ref(&a), "Fehlt", "gtk-4.0"));
        assert!(thema_mit_dunkel(&[a], "Adwaita", "gtk-3.0"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn konfigordner_aus_der_befehlszeile() {
        let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let home = Path::new("/h/.config");
        assert_eq!(
            labwc_konfigordner(&v(&["labwc", "-C", "/h/.config/querencia/labwc"]), home),
            PathBuf::from("/h/.config/querencia/labwc")
        );
        assert_eq!(
            labwc_konfigordner(&v(&["labwc", "--config-dir=/x"]), home),
            PathBuf::from("/x")
        );
        assert_eq!(
            labwc_konfigordner(&v(&["labwc", "--config-dir", "/y", "-s", "foot"]), home),
            PathBuf::from("/y")
        );
        assert_eq!(
            labwc_konfigordner(&v(&["labwc", "-C/z"]), home),
            PathBuf::from("/z")
        );
        assert_eq!(
            labwc_konfigordner(&v(&["labwc", "-s", "-C"]), home),
            PathBuf::from("/h/.config/labwc")
        );
        assert_eq!(
            labwc_konfigordner(&v(&["labwc"]), home),
            PathBuf::from("/h/.config/labwc")
        );
    }

    #[test]
    fn nur_unter_config_home() {
        let home = Path::new("/h/.config");
        assert!(ordner_erlaubt(
            Path::new("/h/.config/querencia/labwc"),
            home
        ));
        assert!(ordner_erlaubt(Path::new("/h/.config/labwc"), home));
        assert!(!ordner_erlaubt(
            Path::new("/h/projects/querencia-runtime/session/querencia/labwc"),
            home
        ));
        assert!(!ordner_erlaubt(Path::new("/etc/xdg/labwc"), home));
        assert!(!ordner_erlaubt(Path::new("relativ/labwc"), home));
        assert!(!ordner_erlaubt(
            Path::new("/h/.config/../projekt/labwc"),
            home
        ));
    }

    #[test]
    fn platzhalter_ist_gueltiges_themerc() {
        for zeile in THEMERC_DUNKEL.lines() {
            let (k, v) = zeile.split_once(": ").expect("Schlüssel: Wert");
            assert!(k.starts_with("window.") || k.starts_with("osd.") || k.starts_with("menu."));
            assert!(v.starts_with('#') || v == "Solid" || v == "Gradient Vertical");
        }
    }
}
