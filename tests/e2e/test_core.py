"""Core: drawing, the four key events, states, apps, notifications, remote page, MQTT + HA discovery.

Run through tests/e2e/run.sh (builds the host d200fw and the headless engine).
Args: <repo root> <work dir>. Needs Python 3 with numpy and Pillow.
"""
import json, os, struct, subprocess, sys, time
import numpy as np
from PIL import Image
P, S = sys.argv[1], sys.argv[2]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from minibroker import Broker

sim = S + "/sim2"; os.makedirs(sim + "/cfg", exist_ok=True)
for f in os.listdir(sim + "/cfg"): os.remove(f"{sim}/cfg/{f}")
for f in ["fb", "kbd"]:
    if os.path.exists(f"{sim}/{f}"): os.remove(f"{sim}/{f}")
open(sim + "/fb", "wb").write(bytes(720 * 1280 * 4)); os.mkfifo(sim + "/kbd")
mark = lambda n: f"{sim}/m_{n}"
for n in "DUAL": 
    if os.path.exists(mark(n)): os.remove(mark(n))

cfg = json.load(open(P + "/app/src-tauri/src/default-config.json"))
cfg["settings"]["follow_active_app"] = False
cfg["settings"]["long_press_ms"] = 400
cfg["settings"]["mqtt"].update(enabled=True, host="127.0.0.1", port=18830)
cfg["profiles"][0]["pages"]["main"]["8"] = {"states": [
    {"label": "Off", "bg": "#ff0000", "actions": {
        "down": [{"type": "command", "command": f"touch {mark('D')}"}],
        "up": [{"type": "command", "command": f"touch {mark('U')}"}],
        "press": [{"type": "command", "command": f"touch {mark('A')}"}, {"type": "state", "to": "next"}],
        "long": [{"type": "command", "command": f"touch {mark('L')}"}]}},
    {"label": "On", "bg": "#00ff00", "actions": {"press": [{"type": "state", "to": "next"}]}}]}
json.dump(cfg, open(sim + "/cfg/deck.json", "w"))

broker = Broker(18830)
dev = subprocess.Popen([os.environ["D200FW_HOST_BIN"], "--no-kill", "--port", "7399",
                        "--fb", sim + "/fb", "--input", sim + "/kbd"], stderr=open(sim + "/devlog", "w"))
kbd = open(sim + "/kbd", "wb", buffering=0)
env = dict(os.environ, D200_DIRECT="1", D200_PORT="7399", D200_MEDIA_FILE=sim + "/no-player.txt")
eng = subprocess.Popen([os.environ.get("D200_HEADLESS", P + "/app/src-tauri/target/debug/examples/headless"), sim + "/cfg/deck.json"], env=env,
                       stdin=subprocess.PIPE, stdout=open(sim + "/eng.out", "w"), stderr=subprocess.STDOUT, text=True)
CODES = [29, 15, 14, 13, 12, 11, 10, 9, 8, 7, 34, 33, 31, 30]
RECT = lambda k: ((11 + 260 * (k % 5), 3 + 259 * (k // 5)) if k < 10 else (11 + 260 * (k - 10), 521))
def snap(name=None):
    a = np.fromfile(sim + "/fb", dtype=np.uint8).reshape(1280, 720, 4)[:, :, [2, 1, 0]]
    L = np.ascontiguousarray(a[::-1].transpose(1, 0, 2))
    if name: Image.fromarray(L).save(f"{S}/{name}.png")
    return L
def px(k, dx=6, dy=6):
    x, y = RECT(k); return tuple(int(v) for v in snap()[y + dy, x + dx])
def key(k, down): kbd.write(struct.pack("<IIHHi", 0, 0, 1, CODES[k], 1 if down else 0))
def tap(k): key(k, 1); time.sleep(0.08); key(k, 0)
results = []
def check(name, ok, extra=""):
    results.append(ok); print(("PASS " if ok else "FAIL ") + name, extra)

time.sleep(4)
snap("v2_main")
check("device drawn (sysmon key not black)", px(0, 60, 60) != (0, 0, 0), px(0, 60, 60))
disc = [t for t in broker.retained if t.startswith("homeassistant/")]
check("HA discovery published", len(disc) >= 30, len(disc))
check("availability online", broker.retained.get("d200/availability", b"").startswith(b"online"), broker.retained.get("d200/availability"))
check("profile state published", broker.retained.get("d200/profile") == b"Default")

# short press: down, up, press fire; long does not; state flips red -> green
check("state 0 is red", px(8)[0] > 200 and px(8)[1] < 60, px(8))
tap(8); time.sleep(0.7)
check("short press: down/up/press ran, long did not",
      all(os.path.exists(mark(n)) for n in "DUA") and not os.path.exists(mark("L")))
check("state advanced to green", px(8)[1] > 200 and px(8)[0] < 60, px(8))
tap(8); time.sleep(0.4)
check("state wrapped back to red", px(8)[0] > 200, px(8))
for n in "DUA": os.remove(mark(n))
key(8, 1); time.sleep(0.7); check("long fires while held", os.path.exists(mark("L"))); key(8, 0); time.sleep(0.5)
check("long press: press did not fire", not os.path.exists(mark("A")) and os.path.exists(mark("U")))
check("key events on MQTT", any(b'"long_press"' in p for p in broker.got("d200/key")))

# sysmon press cycles the view
before = snap()[3:199, 11:207].copy(); tap(0); time.sleep(0.5)
check("sysmon press changes view", (snap()[3:199, 11:207] != before).mean() > 0.05)
# clock press toggles time/date
c0 = snap()[3:199, 791:987].copy(); tap(3); time.sleep(0.5)
check("clock press toggles", (snap()[3:199, 791:987] != c0).mean() > 0.05)
# pomodoro
tap(4); time.sleep(2.3); snap("v2_pomo")
p1 = snap()[3:199, 1051:1247].copy(); time.sleep(1.2)
check("pomodoro running (face updates)", (snap()[3:199, 1051:1247] != p1).any())

# notification via MQTT: status key animates
broker.publish("d200/notify", json.dumps({"title": "Doorbell", "text": "Someone is at the front door, please have a look", "icon": "🔔", "color": "#f59e0b", "style": "flash", "duration": 4}))
status_before = snap()[521:717, 791:1247].copy()
time.sleep(0.5); f1 = snap()[521:717, 791:1247].copy(); time.sleep(0.3); f2 = snap("v2_notify")[521:717, 791:1247].copy()
check("notification animates", (f1 != f2).mean() > 0.01)
time.sleep(1.3); snap("v2_notify2")
time.sleep(3.2); snap("v2_status")
back = snap()[521:717, 791:1247]
check("status key back after notification", (back.astype(int) - status_before.astype(int)).__abs__().sum(axis=2).__gt__(40).mean() < 0.08)

# remote page
broker.publish("d200/remote/2/set", json.dumps({"label": "Kitchen", "icon": "💡", "bg": "#ff0000", "value": "on"}), retain=True)
broker.publish("d200/remote/show", "1"); time.sleep(0.8); snap("v2_remote")
check("remote page shows HA key", px(2)[0] > 200 and px(2)[1] < 60, px(2))
tap(2); time.sleep(0.5)
check("remote press published", b"press" in broker.got("d200/remote/2/action"), broker.got("d200/remote/2/action"))
broker.publish("d200/remote/show", "off"); time.sleep(0.6)
check("left remote page", px(8)[0] > 200, px(8))

# profile switch from HA
broker.publish("d200/profile/set", "Browser"); time.sleep(0.8); snap("v2_browser")
check("profile switched by MQTT", broker.retained.get("d200/profile") == b"Browser")
broker.publish("d200/brightness/set", "77"); time.sleep(0.3)
check("brightness state", broker.retained.get("d200/brightness") == b"77")

# status key press -> home; long -> remote
broker.publish("d200/profile/set", "Default"); time.sleep(0.5)
key(13, 1); time.sleep(0.6); key(13, 0); time.sleep(0.6)
check("status long press opens remote", px(2)[0] > 200, px(2))
tap(13); time.sleep(0.6)
check("status press goes home", px(8)[0] > 200, px(8))

# device goes away -> availability offline
dev.kill(); dev.wait(); time.sleep(1.0)
check("availability offline after disconnect", broker.retained.get("d200/availability") == b"offline", broker.retained.get("d200/availability"))
eng.kill()
print(f"\n{sum(results)}/{len(results)} passed")
err = [l for l in open(sim + "/eng.out") if "panicked" in l.lower()]
print("engine panics:", err[:5])
sys.exit(0 if all(results) else 1)
