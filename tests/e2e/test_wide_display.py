"""Wide display: card priorities, critical cards, printer over MQTT, Steam detection (fake library), square widgets.

Run through tests/e2e/run.sh (builds the host d200fw and the headless engine).
Args: <repo root> <work dir>. Needs Python 3 with numpy and Pillow.
"""
import http.server, json, os, shutil, struct, subprocess, sys, threading, time
import numpy as np
from PIL import Image, ImageDraw
P, S = sys.argv[1], sys.argv[2]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from minibroker import Broker
sim = S + "/cards"; shutil.rmtree(sim, ignore_errors=True); os.makedirs(sim + "/cfg"); os.makedirs(sim + "/www")
open(sim + "/fb", "wb").write(bytes(720 * 1280 * 4)); os.mkfifo(sim + "/kbd")
art = Image.new("RGB", (300, 300)); d = ImageDraw.Draw(art)
for y in range(300):
    for x in range(0, 300, 2):
        v = int(90 + 70 * np.sin(x / 35 + y / 22) * np.cos(y / 50))
        d.rectangle([x, y, x + 1, y], fill=(int(v * .5), v, int(v * .8)))
art.save(sim + "/www/art.png")
class Q(http.server.SimpleHTTPRequestHandler):
    def __init__(s, *a, **k): super().__init__(*a, directory=sim + "/www", **k)
    def log_message(s, *a): pass
threading.Thread(target=http.server.ThreadingHTTPServer(("127.0.0.1", 18992), Q).serve_forever, daemon=True).start()
broker = Broker(18831)
# fake Steam library + game process
lib = sim + "/Steam/steamapps"; os.makedirs(lib + "/common/No Man's Sky/Binaries")
open(lib + "/appmanifest_275850.acf", "w").write('"AppState"\n{\n\t"appid"\t\t"275850"\n\t"name"\t\t"No Man\'s Sky"\n\t"installdir"\t\t"No Man\'s Sky"\n}\n')
shutil.copy(S + "/fakeide/webstorm", lib + "/common/No Man's Sky/Binaries/NMS")
cfg = json.load(open(P + "/app/src-tauri/src/default-config.json"))
cfg["settings"]["follow_active_app"] = False
cfg["settings"]["mqtt"].update(enabled=True, host="127.0.0.1", port=18831)
cfg["settings"]["screensaver"]["enabled"] = False
cfg["status_key"]["app"] = "media"
cfg["profiles"][0]["name"] = "Work"
pg = cfg["profiles"][0]["pages"]["main"]
pg["0"] = {"app": "core:sysmon", "config": {"modes": ["cpu"]}}; pg["1"] = {"app": "core:sysmon", "config": {"modes": ["ram"]}}
pg["2"] = {"app": "core:sysmon", "config": {"modes": ["disk"]}}; pg["3"] = {"app": "core:sysmon", "config": {"modes": ["net"]}}
pg["4"] = {"app": "core:clock"}
json.dump(cfg, open(sim + "/cfg/deck.json", "w"))
open(sim + "/player.txt", "w").write("Spotify\tplaying\tMidnight City\tM83\tHurry Up, We're Dreaming\t84\t243\thttp://127.0.0.1:18992/art.png\n")
dev = subprocess.Popen([os.environ["D200FW_HOST_BIN"], "--no-kill", "--port", "7396", "--fb", sim + "/fb", "--input", sim + "/kbd"], stderr=subprocess.DEVNULL)
kbd = open(sim + "/kbd", "wb", buffering=0)
env = dict(os.environ, D200_DIRECT="1", D200_PORT="7396", D200_MEDIA_FILE=sim + "/player.txt")
eng = subprocess.Popen([os.environ.get("D200_HEADLESS", P + "/app/src-tauri/target/debug/examples/headless"), sim + "/cfg/deck.json"], env=env, stdin=subprocess.PIPE, stdout=open(sim + "/out", "w"), stderr=subprocess.STDOUT, text=True)
def snap():
    a = np.fromfile(sim + "/fb", dtype=np.uint8).reshape(1280, 720, 4)[:, :, [2, 1, 0]]
    return np.ascontiguousarray(a[::-1].transpose(1, 0, 2))
wide = lambda: snap()[521:717, 791:1247].copy()
def key(k, down): kbd.write(struct.pack("<IIHHi", 0, 0, 1, [29, 15, 14, 13, 12, 11, 10, 9, 8, 7, 34, 33, 31, 30][k], 1 if down else 0))
results = []
def check(name, ok, extra=""):
    results.append(ok); print(("PASS " if ok else "FAIL ") + name, extra)
shots = {}
time.sleep(6)
full = snap(); Image.fromarray(full).save(S + "/cards_main.png")
shots["media (default app)"] = wide()
check("media card: cover left 42%", tuple(full[521 + 100, 791 + 60]) != (15, 18, 22))
# push card from HA
broker.publish("d200/notify", json.dumps({"kind": "push", "source": "Home Assistant", "title": "Someone's at the door", "detail": "Front door · Just now", "icon": "ha-security"}))
time.sleep(1.4); shots["push"] = wide()
# lower priority (email) waits behind push; higher (meeting) interrupts
broker.publish("d200/notify", json.dumps({"kind": "email", "source": "New mail", "title": "Alex Morgan", "detail": "Design review", "note": "2 unread", "group": "email"}))
time.sleep(0.6); check("email waits behind push", (wide() == shots["push"]).mean() > 0.6)
broker.publish("d200/notify", json.dumps({"kind": "meeting", "source": "Next meeting", "title": "In 5 min", "detail": "Design sync", "note": "11:00–11:30 · Teams", "url": "https://example.invalid/join"}))
time.sleep(1.4); shots["meeting (interrupts)"] = wide()
check("meeting interrupts push", (shots["meeting (interrupts)"] != shots["push"]).mean() > 0.2)
# long press dismisses the meeting -> push comes back (it was queued)
key(13, 1); time.sleep(0.8); key(13, 0); time.sleep(1.4)
back = wide(); check("long press dismissed meeting, push returns", (back != shots["meeting (interrupts)"]).mean() > 0.2)
key(13, 1); time.sleep(0.8); key(13, 0); time.sleep(1.4); shots["email (queued)"] = wide()
key(13, 1); time.sleep(0.8); key(13, 0); time.sleep(1.4)
# printer via MQTT: printing -> complete makes a card
broker.publish("d200/printer", json.dumps({"state": "printing", "percent": 62, "remaining_min": 18, "name": "Bambu X1C", "job": "bracket"}))
time.sleep(0.5)
broker.publish("d200/printer", json.dumps({"state": "complete", "percent": 100, "name": "Bambu X1C", "job": "bracket"}))
time.sleep(1.4); shots["printer complete"] = wide()
key(13, 1); time.sleep(0.8); key(13, 0); time.sleep(0.6)
# critical stays until dismissed
broker.publish("d200/notify", json.dumps({"kind": "push", "source": "Smoke alarm", "title": "Kitchen smoke detected", "critical": True, "icon": "warning"}))
time.sleep(1.4); shots["critical"] = wide(); time.sleep(9); still = wide()
check("critical card stays beyond 8 s", (still == shots["critical"]).mean() > 0.9)
key(13, 1); time.sleep(0.1); key(13, 0); time.sleep(1.0)
check("short press dismisses critical", (wide() != shots["critical"]).mean() > 0.2)
# Steam: start the fake game
game = subprocess.Popen([lib + "/common/No Man's Sky/Binaries/NMS"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(5); shots["steam launched"] = wide()
check("steam card appears for the game", (shots["steam launched"] != shots["media (default app)"]).mean() > 0.2)
time.sleep(8.5)
eng.stdin.write("state\n"); eng.stdin.flush(); time.sleep(0.5)
st = [l for l in open(sim + "/out") if l.startswith("state ")]
game_name = json.loads(st[-1][6:])["integrations"]["game"] if st else None
check("game detected by name from appmanifest", game_name == "No Man's Sky", game_name)
# square widgets
Image.fromarray(snap()).save(S + "/cards_widgets.png")
game.kill(); subprocess.run(["pkill", "-f", "inspect=127.0.0.1:0"])
eng.kill(); dev.kill()
sheet = Image.new("RGB", (456 * 2 + 30, (196 + 40) * 4), (40, 40, 40)); dd = ImageDraw.Draw(sheet)
for i, (name, im) in enumerate(shots.items()):
    x, y = (i % 2) * (456 + 30), (i // 2) * 236
    sheet.paste(Image.fromarray(im), (x, y + 30)); dd.text((x + 4, y + 8), name, fill=(220, 220, 220))
sheet.save(S + "/cards_sheet.png")
print(f"{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
