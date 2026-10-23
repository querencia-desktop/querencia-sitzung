//! `--grafik-pruefen` (0.1.8, QD-169): welcher OpenGL-Renderer Programme
//! der Sitzung erwartet. Der Sitzungsstarter fragt das vor dem Start von
//! labwc; meldet Mesa zink auf NVK (nouveau), schaltet er für die Sitzung
//! Software-OpenGL ein.
//!
//! Legt ohne Fenster und ohne Wayland einen GLES-Kontext an (EGL, Plattform
//! surfaceless von Mesa), liest `GL_RENDERER` und `GL_VENDOR` und gibt sie
//! als `schlüssel=wert` aus. libEGL wird per `dlopen` geladen, wie PAM und
//! libpolkit: keine Entwicklungsdateien, kein Abbruch, wenn sie fehlt. Der
//! Starter ruft das in einem eigenen Prozess mit Zeitgrenze auf; ein Absturz
//! hier hält die Sitzung nicht auf.

use std::ffi::{CStr, c_char, c_void};
use std::time::Instant;

type EglDisplay = *mut c_void;
type GetProcAddress = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type GetPlatformDisplay = unsafe extern "C" fn(u32, *mut c_void, *const isize) -> EglDisplay;
type Initialize = unsafe extern "C" fn(EglDisplay, *mut i32, *mut i32) -> u32;
type QueryString = unsafe extern "C" fn(EglDisplay, i32) -> *const c_char;
type BindApi = unsafe extern "C" fn(u32) -> u32;
type CreateContext =
    unsafe extern "C" fn(EglDisplay, *mut c_void, *mut c_void, *const i32) -> *mut c_void;
type MakeCurrent = unsafe extern "C" fn(EglDisplay, *mut c_void, *mut c_void, *mut c_void) -> u32;
type DestroyContext = unsafe extern "C" fn(EglDisplay, *mut c_void) -> u32;
type Terminate = unsafe extern "C" fn(EglDisplay) -> u32;
type GetError = unsafe extern "C" fn() -> i32;
type DriverName = unsafe extern "C" fn(EglDisplay) -> *const c_char;
type GetString = unsafe extern "C" fn(u32) -> *const u8;

const EGL_PLATFORM_SURFACELESS_MESA: u32 = 0x31DD;
const EGL_OPENGL_ES_API: u32 = 0x30A0;
const EGL_CONTEXT_CLIENT_VERSION: i32 = 0x3098;
const EGL_NONE: i32 = 0x3038;
const EGL_VENDOR: i32 = 0x3053;
const EGL_EXTENSIONS: i32 = 0x3055;
const GL_VENDOR: u32 = 0x1F00;
const GL_RENDERER: u32 = 0x1F01;
const GL_VERSION: u32 = 0x1F02;

/// Was die Prüfung gefunden hat.
#[derive(Debug, Default)]
pub struct Befund {
    pub renderer: String,
    pub vendor: String,
    pub version: String,
    /// Mesa-Treiber laut `eglGetDisplayDriverName` (z. B. `zink`), falls bekannt.
    pub treiber: Option<String>,
    pub egl_vendor: String,
}

impl Befund {
    /// Ausgabe für den Starter: eine Zeile je Wert, ohne Zeilenumbrüche im Wert.
    pub fn zeilen(&self, dauer_ms: u128) -> String {
        let rein = |s: &str| s.replace(['\n', '\r'], " ");
        let mut text = format!(
            "renderer={}\nvendor={}\nversion={}\n",
            rein(&self.renderer),
            rein(&self.vendor),
            rein(&self.version)
        );
        if let Some(t) = &self.treiber {
            text.push_str(&format!("treiber={}\n", rein(t)));
        }
        text.push_str(&format!(
            "egl={}\ndauer-ms={dauer_ms}\n",
            rein(&self.egl_vendor)
        ));
        text
    }
}

fn text(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        // SAFETY: EGL/GL liefern nullterminierte, statische Zeichenketten.
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }
}

/// Legt den Kontext an und liest den Renderer.
pub fn pruefen() -> Result<Befund, String> {
    // SAFETY: dlopen/dlsym mit gültigen C-Strings; die Signaturen entsprechen
    // EGL 1.5 (egl.h) und GLES 2 (glGetString). Jeder Zeiger wird vor dem
    // Gebrauch auf NULL geprüft.
    unsafe {
        let egl = libc::dlopen(c"libEGL.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if egl.is_null() {
            return Err("libEGL.so.1 nicht ladbar".to_string());
        }
        let sym = |name: &CStr| -> Result<*mut c_void, String> {
            let p = libc::dlsym(egl, name.as_ptr());
            if p.is_null() {
                Err(format!("{} fehlt in libEGL", name.to_string_lossy()))
            } else {
                Ok(p)
            }
        };
        let get_proc: GetProcAddress = std::mem::transmute(sym(c"eglGetProcAddress")?);
        let query: QueryString = std::mem::transmute(sym(c"eglQueryString")?);
        let initialize: Initialize = std::mem::transmute(sym(c"eglInitialize")?);
        let bind_api: BindApi = std::mem::transmute(sym(c"eglBindAPI")?);
        let create_context: CreateContext = std::mem::transmute(sym(c"eglCreateContext")?);
        let make_current: MakeCurrent = std::mem::transmute(sym(c"eglMakeCurrent")?);
        let destroy_context: DestroyContext = std::mem::transmute(sym(c"eglDestroyContext")?);
        let terminate: Terminate = std::mem::transmute(sym(c"eglTerminate")?);
        let get_error: GetError = std::mem::transmute(sym(c"eglGetError")?);

        let client = text(query(std::ptr::null_mut(), EGL_EXTENSIONS)).unwrap_or_default();
        if !client
            .split_whitespace()
            .any(|e| e == "EGL_MESA_platform_surfaceless")
        {
            return Err("EGL ohne EGL_MESA_platform_surfaceless".to_string());
        }
        let platform = {
            let p = libc::dlsym(egl, c"eglGetPlatformDisplay".as_ptr());
            if p.is_null() {
                get_proc(c"eglGetPlatformDisplayEXT".as_ptr())
            } else {
                p
            }
        };
        if platform.is_null() {
            return Err("eglGetPlatformDisplay fehlt".to_string());
        }
        let platform: GetPlatformDisplay = std::mem::transmute(platform);
        let anzeige = platform(
            EGL_PLATFORM_SURFACELESS_MESA,
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if anzeige.is_null() {
            return Err(format!(
                "keine surfaceless-Anzeige (EGL-Fehler {:#x})",
                get_error()
            ));
        }
        let (mut major, mut minor) = (0, 0);
        if initialize(anzeige, &mut major, &mut minor) == 0 {
            return Err(format!(
                "eglInitialize fehlgeschlagen (EGL-Fehler {:#x})",
                get_error()
            ));
        }
        let mut befund = Befund {
            egl_vendor: text(query(anzeige, EGL_VENDOR)).unwrap_or_default(),
            ..Befund::default()
        };
        let treiber = get_proc(c"eglGetDisplayDriverName".as_ptr());
        if !treiber.is_null() {
            let treiber: DriverName = std::mem::transmute(treiber);
            befund.treiber = text(treiber(anzeige));
        }
        let ergebnis = (|| {
            if bind_api(EGL_OPENGL_ES_API) == 0 {
                return Err("eglBindAPI(GLES) fehlgeschlagen".to_string());
            }
            // EGL_KHR_no_config_context und EGL_KHR_surfaceless_context:
            // kein Config, keine Fläche.
            let attribute = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE];
            let kontext = create_context(
                anzeige,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                attribute.as_ptr(),
            );
            if kontext.is_null() {
                return Err(format!(
                    "eglCreateContext fehlgeschlagen (EGL-Fehler {:#x})",
                    get_error()
                ));
            }
            let mut fehler = None;
            if make_current(anzeige, std::ptr::null_mut(), std::ptr::null_mut(), kontext) == 0 {
                fehler = Some(format!(
                    "eglMakeCurrent fehlgeschlagen (EGL-Fehler {:#x})",
                    get_error()
                ));
            } else {
                let mut gs = get_proc(c"glGetString".as_ptr());
                if gs.is_null() {
                    let gles = libc::dlopen(
                        c"libGLESv2.so.2".as_ptr(),
                        libc::RTLD_NOW | libc::RTLD_LOCAL,
                    );
                    if !gles.is_null() {
                        gs = libc::dlsym(gles, c"glGetString".as_ptr());
                    }
                }
                if gs.is_null() {
                    fehler = Some("glGetString fehlt".to_string());
                } else {
                    let gs: GetString = std::mem::transmute(gs);
                    let lesen = |name| text(gs(name).cast::<c_char>()).unwrap_or_default();
                    befund.renderer = lesen(GL_RENDERER);
                    befund.vendor = lesen(GL_VENDOR);
                    befund.version = lesen(GL_VERSION);
                }
                make_current(
                    anzeige,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }
            destroy_context(anzeige, kontext);
            match fehler {
                Some(f) => Err(f),
                None if befund.renderer.is_empty() => Err("GL_RENDERER leer".to_string()),
                None => Ok(()),
            }
        })();
        terminate(anzeige);
        ergebnis.map(|()| befund)
    }
}

/// `--grafik-pruefen`: Ausgabe auf stdout, Exit 0; sonst Meldung, Exit 1.
pub fn ausfuehren() -> i32 {
    let start = Instant::now();
    match pruefen() {
        Ok(befund) => {
            print!("{}", befund.zeilen(start.elapsed().as_millis()));
            0
        }
        Err(e) => {
            eprintln!(
                "querencia-sitzung: Grafikprüfung nach {} ms: {e}",
                start.elapsed().as_millis()
            );
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeilen_ohne_umbrueche() {
        let b = Befund {
            renderer: "zink Vulkan 1.4(NVIDIA GeForce RTX 3060 (NVK GA106) (MESA_NVK))".into(),
            vendor: "Mesa".into(),
            version: "OpenGL ES 3.2 Mesa 26.0.8\nx".into(),
            treiber: Some("zink".into()),
            egl_vendor: "Mesa Project".into(),
        };
        let z = b.zeilen(42);
        assert_eq!(
            z,
            "renderer=zink Vulkan 1.4(NVIDIA GeForce RTX 3060 (NVK GA106) (MESA_NVK))\n\
             vendor=Mesa\nversion=OpenGL ES 3.2 Mesa 26.0.8 x\ntreiber=zink\n\
             egl=Mesa Project\ndauer-ms=42\n"
        );
        let ohne = Befund {
            renderer: "llvmpipe".into(),
            ..Befund::default()
        };
        assert!(!ohne.zeilen(1).contains("treiber="));
    }

    /// Liest nur: Renderer dieses Rechners ohne Fenster (wie der Starter).
    #[test]
    fn pruefen_liefert_renderer_oder_fehler() {
        match pruefen() {
            Ok(b) => assert!(!b.renderer.is_empty()),
            Err(e) => assert!(!e.is_empty()),
        }
    }
}
