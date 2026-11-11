"""Deckling model: deck.json migration, a key whose plugin is missing, bound icons and theme variables.

Run through tests/e2e/run.sh (builds the host d200fw and the headless engine).
Args: <repo root> <work dir>. Needs Python 3 with numpy and Pillow.
"""
import json, os, shutil, struct, subprocess, sys, time
import numpy as np
from PIL import Image
P, S = sys.argv[1], sys.argv[2]

sim = S + "/sim5"; shutil.rmtree(sim, ignore_errors=True); os.makedirs(sim + "/cfg")
open(sim + "/fb", "wb").write(bytes(720 * 1280 * 4)); os.mkfifo(sim + "/kbd")

# A version 1 file (no "version"): old app and action shapes, an icon_state, a plugin
# that is not installed, and a theme variable bound to a signal.
v1 = {
    "settings": {"follow_active_app": False, "screensaver": {"enabled": False},
                 "theme": {"variables": {"accent": {"bind": "time.second", "steps": ["#FF0000", "#0000FF"], "fallback": "#69E0C1"}}}},
    "profiles": [{"name": "Default", "pages": {"main": {
        "0": {"states": [{"label": "Lamp", "icon": "light"}]},
        "1": {"app": "com.example.missing:thing", "config": {"unit": "b"}, "future": True},
        "2": {"app": {"type": "clock", "h12": True}},
        "3": {"states": [{"label": "Hall", "icon": "light", "icon_state": {"on": False},
                          "actions": {"press": [{"type": "command", "command": "true"}]}}]},
    }}}],
}
json.dump(v1, open(sim + "/cfg/deck.json", "w"))

dev = subprocess.Popen([os.environ["D200FW_HOST_BIN"], "--no-kill", "--port", "7399",
                        "--fb", sim + "/fb", "--input", sim + "/kbd"], stderr=open(sim + "/devlog", "w"))
kbd = open(sim + "/kbd", "wb", buffering=0)
env = dict(os.environ, D200_DIRECT="1", D200_PORT="7399", D200_MEDIA_FILE=sim + "/no-player.txt")
eng = subprocess.Popen([os.environ.get("D200_HEADLESS", P + "/app/src-tauri/target/debug/examples/headless"), sim + "/cfg/deck.json"], env=env,
                       stdin=subprocess.PIPE, stdout=open(sim + "/eng.out", "w"), stderr=subprocess.STDOUT, text=True)
RECT = lambda k: ((11 + 260 * (k % 5), 3 + 259 * (k // 5)) if k < 10 else (11 + 260 * (k - 10), 521))
def snap(name=None):
    a = np.fromfile(sim + "/fb", dtype=np.uint8).reshape(1280, 720, 4)[:, :, [2, 1, 0]]
    L = np.ascontiguousarray(a[::-1].transpose(1, 0, 2))
    if name: Image.fromarray(L).save(f"{S}/{name}.png")
    return L
def count(L, k, rgb, tol=60):
    x, y = RECT(k)
    a = L[y:y + 196, x:x + 196].astype(int)
    return int((np.abs(a - list(rgb)).sum(axis=2) < tol).sum())
results = []
def check(name, ok, extra=""):
    results.append(ok); print(("PASS " if ok else "FAIL ") + name, extra)

time.sleep(4.5)
saved = json.load(open(sim + "/cfg/deck.json"))
keys = saved["profiles"][0]["pages"]["main"]
check("migrated to the current version", saved.get("version") == 5, saved.get("version"))
check("settings moved to their plugins", "settings" not in saved and saved["plugins"]["core"]["config"]["screensaver"]["enabled"] is False
      and "accent" in saved["plugins"]["core"]["config"]["theme"]["variables"], list(saved))
check("original kept as deck.v1.json", json.load(open(sim + "/cfg/deck.v1.json")) == v1)
check("app became an id with config", keys["2"] == {"app": "com.deckling.core:clock", "config": {"h12": True}}, keys["2"])
check("actions became refs", keys["3"]["states"][0]["on"]["press"][0] == {"action": "com.deckling.core:command", "params": {"command": "true"}}, keys["3"])
check("icon_state became props", keys["3"]["states"][0]["props"] == {"toggle.on": False}, keys["3"])
check("unknown app key unchanged", keys["1"] == v1["profiles"][0]["pages"]["main"]["1"], keys["1"])

AMBER = (255, 191, 105)
L = snap("v5_main")
check("missing plugin: dashed amber placeholder", count(L, 1, AMBER, 40) > 300, count(L, 1, AMBER, 40))

# accent follows time.second: red on even seconds, blue on odd ones
reds, blues = [], []
for _ in range(4):
    L = snap()
    reds.append(count(L, 0, (255, 0, 0))); blues.append(count(L, 0, (0, 0, 255)))
    time.sleep(0.55)
check("theme variable bound to a signal recolours the icon", max(reds) > 300 and max(blues) > 300, (reds, blues))
check("light switched off by props stays dark", count(snap("v5_off"), 3, (255, 0, 0)) + count(snap(), 3, (0, 0, 255)) < 50)

eng.kill(); dev.kill()
print(f"\n{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
