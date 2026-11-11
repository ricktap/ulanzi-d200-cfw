"""Screensaver: all five scenes, fade-in, deferred notifications, wake without running the key's action.

Run through tests/e2e/run.sh (builds the host d200fw and the headless engine).
Args: <repo root> <work dir>. Needs Python 3 with numpy and Pillow.
"""
import json, os, shutil, struct, subprocess, sys, time
import numpy as np
from PIL import Image
P, S = sys.argv[1], sys.argv[2]
CODES = [29, 15, 14, 13, 12, 11, 10, 9, 8, 7, 34, 33, 31, 30]
def snap(sim):
    a = np.fromfile(sim + "/fb", dtype=np.uint8).reshape(1280, 720, 4)[:, :, [2, 1, 0]]
    return np.ascontiguousarray(a[::-1].transpose(1, 0, 2))
results = []
def check(name, ok, extra=""):
    results.append(ok); print(("PASS " if ok else "FAIL ") + name, extra)
shots = []
for scene in ["snow", "pink", "beach", "leaves", "rain"]:
    sim = f"{S}/saver-{scene}"; shutil.rmtree(sim, ignore_errors=True); os.makedirs(sim + "/cfg")
    open(sim + "/fb", "wb").write(bytes(720 * 1280 * 4)); os.mkfifo(sim + "/kbd")
    cfg = json.load(open(P + "/app/src-tauri/src/default-config.json"))
    cfg["settings"]["follow_active_app"] = False
    cfg["settings"]["screensaver"] = {"enabled": True, "idle_minutes": 30, "scene": scene, "southern": False, "motion": True, "fps": 8}
    marker = sim + "/ran"
    cfg["profiles"][0]["pages"]["main"]["5"]["states"][0]["on"] = {"press": [{"action": "com.deckling.core:command", "params": {"command": f"touch {marker}"}}]}
    json.dump(cfg, open(sim + "/cfg/deck.json", "w"))
    dev = subprocess.Popen([os.environ["D200FW_HOST_BIN"], "--no-kill", "--port", "7397", "--fb", sim + "/fb", "--input", sim + "/kbd"], stderr=subprocess.DEVNULL)
    kbd = open(sim + "/kbd", "wb", buffering=0)
    env = dict(os.environ, D200_DIRECT="1", D200_PORT="7397", D200_MEDIA_FILE=sim + "/none")
    eng = subprocess.Popen([os.environ.get("D200_HEADLESS", P + "/app/src-tauri/target/debug/examples/headless"), sim + "/cfg/deck.json"], env=env, stdin=subprocess.PIPE, stdout=open(sim + "/out", "w"), stderr=subprocess.STDOUT, text=True)
    time.sleep(3.5)
    awake = snap(sim)
    eng.stdin.write("saver on\n"); eng.stdin.flush()
    time.sleep(1.0); mid = snap(sim)
    time.sleep(3.0); a = snap(sim); time.sleep(0.6); b = snap(sim)
    shots.append(a)
    if scene == "leaves":
        check("fade: halfway frame differs from both ends", (mid != awake).any() and (mid != a).any())
        check("scene animates between frames", (a != b).mean() > 0.001, (a != b).mean())
        # gutters stay black, keys show the scene background
        check("gutters black", tuple(a[230, 230]) == (0, 0, 0), tuple(a[230, 230]))
        eng.stdin.write('notify Hello\n'); eng.stdin.flush(); time.sleep(0.8)
        c = snap(sim)
        check("ordinary notification deferred: quiet mint dot", tuple(c[521 + 20, 791 + 456 - 20]) == (105, 224, 193), tuple(c[521 + 20, 791 + 456 - 20]))
        # wake with key 5: must not run its action
        kbd.write(struct.pack("<IIHHi", 0, 0, 1, CODES[5], 1)); time.sleep(0.1); kbd.write(struct.pack("<IIHHi", 0, 0, 1, CODES[5], 0)); time.sleep(1.2)
        w = snap(sim)
        check("woke up (faces back)", (np.abs(w.astype(int) - awake.astype(int)).sum(axis=2) > 30).mean() < 0.2)
        check("waking press did not run the action", not os.path.exists(marker))
        check("deferred notification plays after wake", (w[521:717, 791:1247] != awake[521:717, 791:1247]).any())
        kbd.write(struct.pack("<IIHHi", 0, 0, 1, CODES[5], 1)); time.sleep(0.1); kbd.write(struct.pack("<IIHHi", 0, 0, 1, CODES[5], 0)); time.sleep(0.8)
        check("next press runs the action", os.path.exists(marker))
    eng.kill(); dev.kill(); time.sleep(0.3)
# contact sheet of the five scenes
sheet = Image.new("RGB", (1280, 720 * 5 // 2 + 20), (30, 30, 30))
for i, a in enumerate(shots):
    im = Image.fromarray(a).resize((640, 360))
    sheet.paste(im, ((i % 2) * 640, (i // 2) * 370))
sheet.save(S + "/saver_sheet.png")
print(f"{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
