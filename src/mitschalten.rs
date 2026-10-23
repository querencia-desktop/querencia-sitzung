//! Dunkelmodus für fremde GTK-3-Programme (0.1.5, QD-103), geführt vom
//! Leerlaufdienst.
//!
//! Die Querencia-Programme (GTK 4) folgen dem Farbschema über das
//! Settings-Portal selbst. GTK 3 (3.24 unter Wayland, isoliert gemessen,
//! `nachweis/015-gtk3-messung`) kennt das Farbschema des Portals nicht
//! (Fall m2). Thema und Schrift nimmt es live aus den gsettings, die
//! Dunkelfassung aber nur aus `gtk-application-prefer-dark-theme` in
//! `$XDG_CONFIG_HOME/gtk-3.0/settings.ini`, und das nur beim Start (m3, m4,
//! m10). Ein Abschnitt `[Settings]`, der am Dateiende noch einmal beginnt,
//! geht früheren Einträgen vor (m6). Laufende GTK-3-Programme wechseln
//! deshalb erst beim nächsten Start.
//!
//! Kriterium ist `dunkel_aktiv` wie in Panel und Einstellungen: Farbschema
//! `prefer-dark` **und** ein GTK-Thema mit `gtk-4.0/gtk-dark.css`; dazu muss
//! das Thema `gtk-3.0/gtk-dark.css` haben. Die Datei bekommt dann einen
//! gekennzeichneten Block zwischen [`ANFANG`] und [`ENDE`]; fremde Zeilen
//! bleiben byte-gleich. Bei „hell“ fällt der Block weg, eine Datei, die nur
//! noch aus ihm bestand, wird entfernt. Verweise (Symlinks) und Dateien mit
//! beschädigtem Block bleiben unberührt. Setzt der Benutzer
//! `gtk-application-prefer-dark-theme` in `[Settings]` selbst, gilt sein
//! Eintrag, und die Sitzung schreibt keinen Block.
//!
//! Die labwc-Fensterrahmen schaltet die Sitzung nicht: Ein dunkles
//! labwc-Thema wählen die Einstellungen in `rc.xml` (Entscheidung der
//! Startsession, 22.10.2026).

use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::prelude::*;

use crate::aktion::protokoll;

pub const ANFANG: &str =
    "# >>> querencia-sitzung: Dunkelmodus (von der Sitzung verwaltet, nicht von Hand ändern)";
pub const ENDE: &str = "# <<< querencia-sitzung";

/// Block für `gtk-3.0/settings.ini`. `[Settings]` beginnt neu, weil davor
/// ein anderer Abschnitt stehen kann (Messung m6).
pub const GTK3_BLOCK: &str = "[Settings]\ngtk-application-prefer-dark-theme=true\n";

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

/// Schlüssel, den der Block setzt.
pub const SCHLUESSEL: &str = "gtk-application-prefer-dark-theme";

/// Setzt `text` (ohne verwalteten Block) [`SCHLUESSEL`] selbst im Abschnitt
/// `[Settings]`? Dann gilt der Eintrag des Benutzers, und die Sitzung
/// schreibt keinen Block (kein zweiter Eintrag, der ihn überstimmt).
pub fn eigener_eintrag(text: &str) -> bool {
    let mut abschnitt = "";
    for zeile in text.lines() {
        let z = zeile.trim();
        if z.starts_with('#') || z.is_empty() {
            continue;
        }
        if let Some(name) = z.strip_prefix('[').and_then(|z| z.strip_suffix(']')) {
            abschnitt = name.trim();
        } else if abschnitt == "Settings"
            && z.split_once('=')
                .is_some_and(|(k, _)| k.trim() == SCHLUESSEL)
        {
            return true;
        }
    }
    false
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
}

pub fn soll(lage: Lage) -> Soll {
    let dunkel_aktiv = lage.schema_dunkel && lage.thema_gtk4_dunkel;
    Soll {
        dunkel_aktiv,
        gtk3: dunkel_aktiv && lage.thema_gtk3_dunkel,
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

fn melden(was: &str, pfad: &Path, r: Result<Abgleich, String>) {
    match r {
        Ok(Abgleich::Unveraendert) => {}
        Ok(Abgleich::Geschrieben) => {
            protokoll(&format!(
                "Dunkelmodus: {was} {} geschrieben",
                pfad.display()
            ));
        }
        Ok(Abgleich::Entfernt) => {
            protokoll(&format!("Dunkelmodus: {was} {} entfernt", pfad.display()));
        }
        Err(e) => protokoll(&format!("Dunkelmodus: {was}: {e}")),
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
    let eigen = std::fs::read_to_string(&ini)
        .ok()
        .and_then(|t| block_einsetzen(&t, None).ok())
        .is_some_and(|t| eigener_eintrag(&t));
    if soll.gtk3 && eigen {
        protokoll(&format!(
            "Dunkelmodus: GTK 3: {} setzt {SCHLUESSEL} selbst, der Eintrag des Benutzers gilt (kein Block)",
            ini.display()
        ));
    }
    melden(
        "GTK 3",
        &ini,
        datei_abgleichen(&ini, (soll.gtk3 && !eigen).then_some(GTK3_BLOCK)),
    );
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
    fn eintrag_des_benutzers_wird_erkannt() {
        assert!(!eigener_eintrag(""));
        assert!(!eigener_eintrag("[Settings]\ngtk-font-name=X 9\n"));
        assert!(eigener_eintrag(
            "[Settings]\ngtk-application-prefer-dark-theme=false\n"
        ));
        assert!(eigener_eintrag(
            "[Settings]\n gtk-application-prefer-dark-theme = 1\n"
        ));
        // auskommentiert oder in einem anderen Abschnitt zählt nicht
        assert!(!eigener_eintrag(
            "[Settings]\n#gtk-application-prefer-dark-theme=1\n"
        ));
        assert!(!eigener_eintrag(
            "[Andere]\ngtk-application-prefer-dark-theme=1\n"
        ));
        assert!(eigener_eintrag(
            "[Andere]\nx=1\n[Settings]\ngtk-application-prefer-dark-theme=1\n"
        ));
        // der eigene Block zählt nicht, er wird vorher herausgenommen
        let mit = block_einsetzen("[Settings]\n", Some(GTK3_BLOCK)).unwrap();
        assert!(!eigener_eintrag(&block_einsetzen(&mit, None).unwrap()));
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
        };
        assert_eq!(soll(l(false, true, true)), hell);
        // Thema ohne Dunkelfassung (Querencia-Klassisch 0.1): alles hell
        assert_eq!(soll(l(true, false, true)), hell);
        assert_eq!(
            soll(l(true, true, true)),
            Soll {
                dunkel_aktiv: true,
                gtk3: true
            }
        );
        // GTK 3 nur mit gtk-3.0/gtk-dark.css
        assert_eq!(
            soll(l(true, true, false)),
            Soll {
                dunkel_aktiv: true,
                gtk3: false
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
        assert!(!thema_mit_dunkel(
            std::slice::from_ref(&a),
            "Fehlt",
            "gtk-4.0"
        ));
        assert!(thema_mit_dunkel(&[a], "Adwaita", "gtk-3.0"));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
