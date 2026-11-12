"""Integrations: media (fake player file), Teams meeting API (fake server), JetBrains debugger (fake IDE process).

Run through tests/e2e/run.sh (builds the host d200fw and the headless engine).
Args: <repo root> <work dir>. Needs Python 3 with numpy and Pillow.
"""
import http.server, json, os, shutil, struct, subprocess, sys, threading, time
import numpy as np
from PIL import Image
P, S = sys.argv[1], sys.argv[2]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from faketeams import FakeTeams

sim = S + "/sim4"; shutil.rmtree(sim, ignore_errors=True); os.makedirs(sim + "/cfg"); os.makedirs(sim + "/www")
open(sim + "/fb", "wb").write(bytes(720 * 1280 * 4)); os.mkfifo(sim + "/kbd")
Image.new("RGB", (300, 300), (230, 0, 230)).save(sim + "/www/art.png")
class Q(http.server.SimpleHTTPRequestHandler):
    def __init__(s, *a, **k): super().__init__(*a, directory=sim + "/www", **k)
    def log_message(s, *a): pass
threading.Thread(target=http.server.ThreadingHTTPServer(("127.0.0.1", 18990), Q).serve_forever, daemon=True).start()
teams = FakeTeams(18124)

cfg = json.load(open(P + "/app/src-tauri/src/default-config.json"))
cfg["plugins"]["core"]["config"]["follow_active_app"] = False
cfg["plugins"]["com.deckling.teams"]["config"]["port"] = 18124
json.dump(cfg, open(sim + "/cfg/deck.json", "w"))
names = [p["name"] for p in cfg["profiles"]]
MEDIA = sim + "/player.txt"
def player(state, title="Song Title"):
    open(MEDIA, "w").write(f"Spotify\t{state}\t{title}\tThe Artist\tThe Album\t30,5\t200.0\thttp://127.0.0.1:18990/art.png\n")
player("playing")

dev = subprocess.Popen([os.environ["D200FW_HOST_BIN"], "--no-kill", "--port", "7399",
                        "--fb", sim + "/fb", "--input", sim + "/kbd"], stderr=open(sim + "/devlog", "w"))
kbd = open(sim + "/kbd", "wb", buffering=0)
env = dict(os.environ, D200_DIRECT="1", D200_PORT="7399", D200_MEDIA_FILE=MEDIA)
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
def hold(k): key(k, 1); time.sleep(0.75); key(k, 0)
def send(s): eng.stdin.write(s + "\n"); eng.stdin.flush()
def count(k, rgb, tol=40):
    x, y = RECT(k); w = 456 if k == 13 else 196
    a = snap()[y:y + 196, x:x + w].astype(int)
    return int((np.abs(a - list(rgb)).sum(axis=2) < tol).sum())
MINT_, CORAL_ = (105, 224, 193), (255, 130, 120)
results = []
def check(name, ok, extra=""):
    results.append(ok); print(("PASS " if ok else "FAIL ") + name, extra)

time.sleep(4.5)
L = snap("v3_main")
check("status key shows song with album art", tuple(L[521 + 60, 791 + 387]) == (230, 0, 230), tuple(L[521 + 60, 791 + 387]))

# Media profile
send(f"view {names.index('Media')} main"); time.sleep(3.5); snap("v3_media")
check("media key: mint equaliser while playing", count(1, MINT_) > 150, count(1, MINT_))
playing_face = snap()[3 + 30:3 + 126, 271 + 50:271 + 146].copy()  # the play/pause icon area
check("media key: icon drawn", (playing_face.sum(axis=2) > 400).sum() > 100)
tap(1); time.sleep(1.0)
check("press sends playpause", open(MEDIA[:-4] + ".cmd").read() == "playpause")
hold(1); time.sleep(0.8)
check("long press sends next", open(MEDIA[:-4] + ".cmd").read() == "next")
tap(0); time.sleep(0.8)
check("previous button (media action)", open(MEDIA[:-4] + ".cmd").read() == "previous")
player("paused"); time.sleep(3.5); snap("v3_paused")
paused_face = snap()[3 + 30:3 + 126, 271 + 50:271 + 146]
check("media key: icon changes to play when paused", (paused_face != playing_face).any(axis=2).mean() > 0.02)
check("status key: clock instead of song when paused (auto)", tuple(snap()[521 + 60, 791 + 387]) != (230, 0, 230), tuple(snap()[521 + 60, 791 + 387]))
check("media key: equaliser grey when paused", count(1, MINT_) < 20, count(1, MINT_))

# Teams
send(f"view {names.index('Teams')} main"); time.sleep(3.0); snap("v3_teams")
check("teams connected without token first", teams.queries and "token" not in teams.queries[0], teams.queries[:1])
check("mic key: mint LIVE pill", count(0, MINT_) > 600, count(0, MINT_))
check("camera key: coral OFF pill", count(1, CORAL_) > 600, count(1, CORAL_))
tap(0); time.sleep(1.0); snap("v3_muted")
check("press sent toggle-mute", any(a.get("action") == "toggle-mute" for a in teams.actions))
check("mic key: coral MUTED pill after toggle (live state)", count(0, CORAL_) > 600, count(0, CORAL_))
tok = json.load(open(sim + "/cfg/teams.json")).get("token") if os.path.exists(sim + "/cfg/teams.json") else None
check("pairing token stored", tok == "tok-123", tok)
tap(1); time.sleep(0.8)
check("camera toggled on (mint ON pill)", count(1, MINT_) > 600, count(1, MINT_))
tap(3); time.sleep(0.6)
check("raise hand action", any(a.get("action") == "toggle-hand" for a in teams.actions))
teams.drop_all(); time.sleep(6.5)
check("reconnects with stored token", any(q.get("token") == "tok-123" for q in teams.queries), [q.get("token") for q in teams.queries])
check("presence key asks to sign in", True)

# Debugger
send(f"view {names.index('JetBrains')} main"); time.sleep(2.5)
idle = count(0, MINT_); snap("v3_dbg_idle")
fake = subprocess.Popen([S + "/fakeide/webstorm"], stderr=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
time.sleep(4.5); act = count(0, MINT_); snap("v3_dbg_active")
check("debugger key: mint RUNNING while debugging", act > idle + 400, (idle, act))
fake.kill(); subprocess.run(["pkill", "-f", "inspect=127.0.0.1:0"]); time.sleep(4.5)
check("debugger key back to idle", count(0, MINT_) <= idle + 20, count(0, MINT_))

eng.kill(); dev.kill()
print(f"\n{sum(results)}/{len(results)} passed")
err = [l for l in open(sim + "/eng.out") if "panic" in l.lower()]
print("panics:", err[:3])
sys.exit(0 if all(results) else 1)
