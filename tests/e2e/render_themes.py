"""Renders the default and an example theme (Compact vs Airy) to PNGs in the work dir. Args: <repo> <work> <default|themed> <port> <broker port>.

Run through tests/e2e/run.sh (builds the host d200fw and the headless engine).
Args: <repo root> <work dir>. Needs Python 3 with numpy and Pillow.
"""
import http.server, json, os, shutil, struct, subprocess, sys, threading, time
import numpy as np
from PIL import Image, ImageDraw
P, S, variant, port, bport = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from minibroker import Broker
sim = f"{S}/theme-{variant}"; shutil.rmtree(sim, ignore_errors=True); os.makedirs(sim + "/cfg")
open(sim + "/fb", "wb").write(bytes(720 * 1280 * 4)); os.mkfifo(sim + "/kbd")
art = Image.new("RGB", (300, 300)); d = ImageDraw.Draw(art)
for yy in range(300):
    for xx in range(0, 300, 2):
        v = int(90 + 70 * np.sin(xx / 35 + yy / 22) * np.cos(yy / 50))
        d.rectangle([xx, yy, xx + 1, yy], fill=(int(v * .5), v, int(v * .8)))
art.save(sim + "/art.png")
class Q(http.server.SimpleHTTPRequestHandler):
    def __init__(s, *a, **k): super().__init__(*a, directory=sim, **k)
    def log_message(s, *a): pass
srv = http.server.ThreadingHTTPServer(("127.0.0.1", port + 1000), Q); threading.Thread(target=srv.serve_forever, daemon=True).start()
broker = Broker(bport)
cfg = json.load(open(P + "/app/src-tauri/src/default-config.json"))
cfg["settings"]["follow_active_app"] = False
cfg["settings"]["mqtt"].update(enabled=True, host="127.0.0.1", port=bport)
cfg["settings"]["screensaver"]["enabled"] = False
cfg["status_key"]["app"] = "media"
pg = cfg["profiles"][0]["pages"]["main"]
pg["0"] = {"app": "core:sysmon", "config": {"modes": ["cpu"]}}; pg["1"] = {"app": "core:sysmon", "config": {"modes": ["ram"]}}
pg["2"] = {"app": "core:sysmon", "config": {"modes": ["disk"]}}; pg["3"] = {"app": "core:sysmon", "config": {"modes": ["net"]}}; pg["4"] = {"app": "core:clock"}
pg["5"] = {"states": [{"label": "Desk", "icon": "core:light", "props": {"toggle.on": True, "rgb.color": "#B79AF7"}}]}
pg["6"] = {"states": [{"label": "Hall", "icon": "core:light", "props": {"toggle.on": False}}]}
pg["7"] = {"states": [{"label": "Office", "icon": "core:thermostat", "props": {"label.text": "21.5", "progress.value": 55}}]}
pg["8"] = {"states": [{"label": "Focus", "icon": "core:pomodoro", "props": {"progress.value": 40}}]}
pg["9"] = {"states": [{"label": "AMS", "icon": "core:bambu-ams-lite", "props": {f"com.ricktap.bambu:ams.slot{i+1}_{k}": v for i, (c, r) in enumerate([("#69E0C1", 80), ("#B79AF7", 30), ("#FFBF69", 60), ("#FF8278", 95)]) for k, v in (("color", c), ("remaining", r))}}]}
if variant == "themed":
    cfg["settings"]["theme"] = {
        "palette": {"active": "#74b9ff", "info": "#c7a8ff", "attention": "#ffd166", "error": "#ff6b81", "background": "#101820"},
        "widgets": {"music": {"accent": "#c7a8ff", "light": "#8970dd", "light_opacity": 0.65, "layout": "airy"},
                    "network": {"network_up": "#69e0c1", "network_down": "#74b9ff"},
                    "mail": {"layout": "airy"}, "printer": {"layout": "airy"}}}
json.dump(cfg, open(sim + "/cfg/deck.json", "w"))
open(sim + "/player.txt", "w").write(f"Spotify\tplaying\tMidnight City\tM83\tHurry Up, We're Dreaming\t84\t243\thttp://127.0.0.1:{port + 1000}/art.png\n")
dev = subprocess.Popen([os.environ["D200FW_HOST_BIN"], "--no-kill", "--port", str(port), "--fb", sim + "/fb", "--input", sim + "/kbd"], stderr=subprocess.DEVNULL)
kbd = open(sim + "/kbd", "wb", buffering=0)
env = dict(os.environ, D200_DIRECT="1", D200_PORT=str(port), D200_MEDIA_FILE=sim + "/player.txt")
eng = subprocess.Popen([os.environ.get("D200_HEADLESS", P + "/app/src-tauri/target/debug/examples/headless"), sim + "/cfg/deck.json"], env=env, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, text=True)
def snap():
    a = np.fromfile(sim + "/fb", dtype=np.uint8).reshape(1280, 720, 4)[:, :, [2, 1, 0]]
    return np.ascontiguousarray(a[::-1].transpose(1, 0, 2))
time.sleep(6.5)
Image.fromarray(snap()).save(f"{S}/theme_{variant}_main.png")
wides = []
wides.append(snap()[521:717, 791:1247].copy())
broker.publish("d200/notify", json.dumps({"kind": "email", "source": "Inbox", "title": "2 unread", "detail": "Alex Morgan · Design review", "note": "Last message 1m ago"}))
time.sleep(1.5); wides.append(snap()[521:717, 791:1247].copy())
kbd.write(struct.pack("<IIHHi", 0, 0, 1, 30, 1)); time.sleep(0.8); kbd.write(struct.pack("<IIHHi", 0, 0, 1, 30, 0)); time.sleep(0.4)
broker.publish("d200/printer", json.dumps({"state": "printing", "percent": 40, "name": "Bambu X1C", "job": "a"}))
time.sleep(0.3)
broker.publish("d200/notify", json.dumps({"kind": "printer", "source": "Bambu X1C", "title": "62%", "detail": "Printing", "note": "18m remaining", "progress": 0.62, "icon": "bambu-ams", "icon_state": {"spools": [{"color": "#69E0C1", "progress": 80}, {"color": "#B79AF7", "progress": 65}, {"color": "#FFBF69", "progress": 40}, {"color": "#FF8278", "progress": 95}]}}))
time.sleep(1.5); wides.append(snap()[521:717, 791:1247].copy())
eng.kill(); dev.kill(); srv.shutdown()
sheet = Image.new("RGB", (456 * 3 + 40, 196), (40, 40, 40))
for i, w in enumerate(wides):
    sheet.paste(Image.fromarray(w), (i * (456 + 20), 0))
sheet.save(f"{S}/theme_{variant}_wide.png")
print("done", variant)
