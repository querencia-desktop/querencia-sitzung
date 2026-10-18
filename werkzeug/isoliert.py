#!/usr/bin/env python3
"""Querencia Sitzung (G2-K): isolierte GUI-Testumgebung nach dem Muster des
Editor-Prototyps (W5, HMP-04), ohne Berührung der laufenden Sitzung.

Startet einen eigenen labwc 0.9.8 mit headless-Backend in einem eigenen
XDG_RUNTIME_DIR (Name qed-runtime-*, damit qeingabe mitspielt), mit eigenem
D-Bus (dbus-run-session) und eigenem XDG-Profil. Beendet wird ausschließlich
der selbst gestartete Prozessbaum, nie der labwc der Sitzung
(XDG_RUNTIME_DIR unter /run/user/).

Aufruf:  isoliert.py <profil> <skript> [zeitlimit] [anzahl-ausgaenge]
"""

import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time

LABWC = "~/opt/labwc-0.9.8/bin/labwc"
HIER = os.path.dirname(os.path.abspath(__file__))
LAUFZEIT = os.path.join(os.path.dirname(HIER), "laufzeit")


def _session_labwc(pid: str) -> bool:
    try:
        if os.readlink(f"/proc/{pid}/exe") != LABWC:
            return False
        with open(f"/proc/{pid}/environ", "rb") as f:
            env = f.read().split(b"\0")
        return any(e.startswith(b"XDG_RUNTIME_DIR=/run/user/") for e in env)
    except OSError:
        return False


SESSION_LABWC_PIDS = {int(p) for p in os.listdir("/proc") if p.isdigit() and _session_labwc(p)}

RC_XML = """<?xml version="1.0"?>
<labwc_config>
  <core><decoration>server</decoration></core>
  <theme><name>Querencia-Klassisch</name></theme>
  <!-- Vorschlag aus integration/rc.xml-sitzung.patch: Dialog mittig -->
  <windowRules>
    <windowRule identifier="org.querencia.Sitzung*" skipTaskbar="yes" skipWindowSwitcher="yes">
      <action name="AutoPlace" policy="center" />
    </windowRule>
  </windowRules>
</labwc_config>
"""


def main() -> int:
    profile = os.path.abspath(sys.argv[1])
    script = os.path.abspath(sys.argv[2])
    timeout = int(sys.argv[3]) if len(sys.argv) > 3 else 120
    outputs = sys.argv[4] if len(sys.argv) > 4 else "1"
    for sub in ("config/labwc", "data/themes", "cache", "state", "logs"):
        os.makedirs(os.path.join(profile, sub), exist_ok=True)
    rc = os.path.join(profile, "config/labwc/rc.xml")
    if not os.path.exists(rc):
        with open(rc, "w") as f:
            f.write(RC_XML)
    theme = os.path.join(profile, "data/themes/Querencia-Klassisch")
    live_theme = "~/.local/share/themes/Querencia-Klassisch"
    if not os.path.exists(theme) and os.path.isdir(live_theme):
        os.symlink(os.path.realpath(live_theme), theme)
    os.makedirs(LAUFZEIT, exist_ok=True)
    runtime = tempfile.mkdtemp(prefix="qed-runtime-", dir=LAUFZEIT)
    os.chmod(runtime, 0o700)

    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("WAYLAND", "DBUS", "DISPLAY", "XDG_", "LABWC", "SWAYSOCK"))}
    env.update({
        "HOME": "~",
        "XDG_RUNTIME_DIR": runtime,
        "XDG_CONFIG_HOME": os.path.join(profile, "config"),
        "XDG_DATA_HOME": os.path.join(profile, "data"),
        "XDG_CACHE_HOME": os.path.join(profile, "cache"),
        "XDG_STATE_HOME": os.path.join(profile, "state"),
        "XDG_DATA_DIRS": "/usr/share",
        "XDG_CURRENT_DESKTOP": "X-Querencia",
        "XDG_SESSION_TYPE": "wayland",
        "WLR_BACKENDS": "headless",
        "WLR_RENDERER": "pixman",
        "WLR_HEADLESS_OUTPUTS": outputs,
        "WLR_LIBINPUT_NO_DEVICES": "1",
        "XKB_DEFAULT_LAYOUT": "de",
        "LANG": "en_US.UTF-8",
        "LANGUAGE": "de",
        "QS_PROFILE": profile,
        # Sicherheitsnetz: in der isolierten Prüfung nie Neustart,
        # Herunterfahren oder labwc --exit ausführen.
        "QS_TROCKENLAUF": "1",
    })
    log = open(os.path.join(profile, "logs", f"lauf-{time.strftime('%Y%m%d-%H%M%S')}.log"), "w")
    cmd = ["dbus-run-session", "--", LABWC, "-C", os.path.join(profile, "config/labwc"),
           "-s", f"/bin/bash {script}"]
    print("Start:", " ".join(cmd), file=log, flush=True)
    proc = subprocess.Popen(cmd, env=env, stdin=subprocess.DEVNULL, stdout=log,
                            stderr=subprocess.STDOUT, start_new_session=True, cwd=profile)
    marker = os.path.join(profile, "logs", "FERTIG")
    if os.path.exists(marker):
        os.remove(marker)
    deadline = time.time() + timeout
    while time.time() < deadline and proc.poll() is None and not os.path.exists(marker):
        time.sleep(0.5)
    status = "fertig" if os.path.exists(marker) else ("beendet" if proc.poll() is not None else "Zeitlimit")
    pgid = os.getpgid(proc.pid) if proc.poll() is None else None
    if pgid is not None:
        assert pgid != os.getpgid(0)
        for pid in SESSION_LABWC_PIDS:
            try:
                assert os.getpgid(pid) != pgid, "Sitzungs-labwc in eigener Gruppe?!"
            except ProcessLookupError:
                pass
        os.killpg(pgid, signal.SIGTERM)
        try:
            proc.wait(10)
        except subprocess.TimeoutExpired:
            os.killpg(pgid, signal.SIGKILL)
            proc.wait(5)
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            with open(f"/proc/{p}/environ", "rb") as f:
                if f"XDG_RUNTIME_DIR={runtime}".encode() in f.read().split(b"\0"):
                    os.kill(int(p), signal.SIGTERM)
        except (OSError, ProcessLookupError):
            pass
    time.sleep(1)
    shutil.rmtree(runtime, ignore_errors=True)
    for pid in SESSION_LABWC_PIDS:
        assert os.path.exists(f"/proc/{pid}"), "Sitzungs-labwc verschwunden!"
    print(f"Status: {status}; Log: {log.name}; Sitzungs-labwc {sorted(SESSION_LABWC_PIDS)} läuft weiter")
    log.close()
    return 0 if status == "fertig" else 1


if __name__ == "__main__":
    sys.exit(main())
