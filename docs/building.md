# Bauen

Ubuntu 26.04: `cargo`, `rustc` (≥ 1.92, Ubuntu 1.93.1), `pkgconf`,
`libgtk-4-dev`, `libwayland-dev`, `libxkbcommon-dev`, Cairo/Pango. PAM
(`libpam.so.0`) und libpolkit-agent-1 werden zur Laufzeit per `dlopen`
geladen, Entwicklungsdateien sind nicht nötig. Bau aus dem lokalen
Crate-Cache:

```bash
cargo build --release --locked --offline
```

Ergebnis: `target/release/querencia-sitzung` und
`target/release/querencia-polkit-agent`. Für Pakete und Anhänge mit
`RUSTFLAGS="--remap-path-prefix=$HOME=~"` bauen, damit keine Pfade des
Bauordners in den Binaries stehen.

**Übersetzungen (ab 0.1.2):** Beide Programme suchen
`<lang>/LC_MESSAGES/querencia-sitzung.mo` in `<Ordner der Binary>/share/locale`,
dann `<Ordner der Binary>/../share/locale`, zuletzt `/usr/share/locale`
(`QUERENCIA_SITZUNG_LOCALEDIR` setzt den Ordner ausdrücklich). Den Katalog
erzeugt GNU gettext oder das Werkzeug des Repos:

```bash
msgfmt -c -o share/locale/de/LC_MESSAGES/querencia-sitzung.mo po/de.po
python3 werkzeug/i18n.py mo <ziel>        # <ziel>/<lang>/LC_MESSAGES/querencia-sitzung.mo (de it es fr)
```

Ohne Katalog ist die Oberfläche englisch. `i18n::init()` setzt die Locale
als Erstes in `main` (die Dialogtexte entstehen vor dem Start von GTK, der
Sperrbildschirm läuft ohne GTK).

**Symbol:** `data/icons/hicolor/…`, kein `48x48` neben Scalable
(GTK-4.22-Symbolabsturz).

**PAM-Dienst:** Gibt es `/etc/pam.d/querencia-sitzung`, prüft der
Sperrbildschirm damit, sonst mit `login`. Die Datei legt der Administrator
an; ein Vorschlag ohne `pam_nologin` gehört zur Integration auf der
Entwicklungs-VM.

**Integration (nur auf der Entwicklungs-VM, nicht Teil der Veröffentlichung):**
Skripte unter `integration/` stellen die unveränderliche Ablage
`~/opt/querencia/sitzung/<sha16>/` (beide Programme, `share/locale`) und die
Links `~/opt/querencia/bin/querencia-sitzung` und
`…/querencia-polkit-agent` um. Sie starten und beenden keinen Prozess;
laufende Dienste der Sitzung bleiben bis zur nächsten Anmeldung. Der Ordner
enthält feste Pfade der VM und wird vor der Veröffentlichung herausgefiltert.
