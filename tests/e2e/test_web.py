"""The web API plugin (com.deckling.web) against the real engine and a simulated D200:
token, keys by id drawn on the device, signals, notifications, presses, actions.

Run through tests/e2e/run.sh. Args: <repo root> <work dir>. Needs numpy and Pillow.
"""
import json, os, shutil, subprocess, sys, time, urllib.request, urllib.error
import numpy as np
from PIL import Image
P, S = sys.argv[1], sys.argv[2]
PORT, TOKEN = 18765, "e2e-secret-token"

sim = S + "/simweb"; shutil.rmtree(sim, ignore_errors=True); os.makedirs(sim + "/cfg")
open(sim + "/fb", "wb").write(bytes(720 * 1280 * 4)); os.mkfifo(sim + "/kbd")
marker = sim + "/pressed"
cfg = {
    "version": 6,
    "plugins": {
        "core": {"config": {"follow_active_app": False, "screensaver": {"enabled": False}}},
        "com.deckling.web": {"config": {"serve": True, "port": PORT, "token": TOKEN}},
    },
    "profiles": [{"name": "Default", "apps": [], "pages": {"main": {
        "0": {"app": "com.deckling.web:key", "config": {"id": "build"}},
        "1": {"states": [{"label": "Mark", "on": {"press": [{"action": "com.deckling.core:command", "params": {"command": f"touch {marker}"}}]}}]},
    }, "more": {}}}],
}
json.dump(cfg, open(sim + "/cfg/deck.json", "w"))

dev = subprocess.Popen([os.environ["D200FW_HOST_BIN"], "--no-kill", "--port", "7396",
                        "--fb", sim + "/fb", "--input", sim + "/kbd"], stderr=open(sim + "/devlog", "w"))
kbd = open(sim + "/kbd", "wb", buffering=0)
env = dict(os.environ, D200_DIRECT="1", D200_PORT="7396", D200_MEDIA_FILE=sim + "/no-player.txt")
eng = subprocess.Popen([os.environ.get("D200_HEADLESS", P + "/app/src-tauri/target/debug/examples/headless"), sim + "/cfg/deck.json"], env=env,
                       stdin=subprocess.PIPE, stdout=open(sim + "/eng.out", "w"), stderr=subprocess.STDOUT, text=True)

def snap(name=None):
    a = np.fromfile(sim + "/fb", dtype=np.uint8).reshape(1280, 720, 4)[:, :, [2, 1, 0]]
    L = np.ascontiguousarray(a[::-1].transpose(1, 0, 2))
    if name: Image.fromarray(L).save(f"{S}/{name}.png")
    return L
def count(L, k, rgb, tol=40):
    x, y = (11 + 260 * (k % 5), 3 + 259 * (k // 5))
    a = L[y:y + 196, x:x + 196].astype(int)
    return int((np.abs(a - list(rgb)).sum(axis=2) < tol).sum())

def api(method, path, body=None, token=TOKEN):
    data = None if body is None else (body if isinstance(body, bytes) else json.dumps(body).encode())
    req = urllib.request.Request(f"http://127.0.0.1:{PORT}{path}", data=data, method=method)
    if token: req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=5) as r:
            return r.status, json.loads(r.read() or b"null")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"null")

results = []
def check(name, ok, extra=""):
    results.append(ok); print(("PASS " if ok else "FAIL ") + name, extra)

time.sleep(4)
try:
    code, _ = api("GET", "/api", token=None)
    check("no token: 401", code == 401, code)
    code, _ = api("GET", "/api", token="wrong")
    check("wrong token: 401", code == 401, code)
    code, info = api("GET", "/api")
    check("GET /api", code == 200 and info.get("name") == "D200 Deck", info)

    # a key described over HTTP is drawn on the device with its background
    code, _ = api("PUT", "/api/keys/build", {"label": "Build", "kind": "sensor", "value": "red", "background": "#C81E1E"})
    time.sleep(2.0)
    L = snap("web-key")
    check("PUT /api/keys/build draws key 0", code == 200 and count(L, 0, (0xC8, 0x1E, 0x1E)) > 5000, count(L, 0, (0xC8, 0x1E, 0x1E)))
    code, keys = api("GET", "/api/keys")
    check("GET /api/keys", keys == {"build": "Build"}, keys)

    code, _ = api("PUT", "/api/signals/ci.status", {"value": "failed"})
    code, sig = api("GET", "/api/signals/com.deckling.web:ci.status")
    check("signal set and read back", sig.get("value") == "failed", sig)
    code, allsig = api("GET", "/api/signals")
    check("GET /api/signals has the clock", "time.clock" in allsig and "com.deckling.web:build.label" in allsig, len(allsig))

    # press key 1 over HTTP: its own action runs
    code, _ = api("POST", "/api/keys/1/press")
    time.sleep(1.5)
    check("POST /api/keys/1/press runs the key's action", code == 200 and os.path.exists(marker), code)

    # actions: core:page is fine, a command needs permission
    code, _ = api("POST", "/api/actions/com.deckling.core:command", {"params": {"command": f"touch {marker}2"}})
    time.sleep(0.5)
    check("command action refused without permission", code == 403 and not os.path.exists(marker + "2"), code)
    code, _ = api("POST", "/api/actions/core:page", {"params": {"page": "more"}})
    eng.stdin.write("state\n"); eng.stdin.flush(); time.sleep(1.0)
    st = [l for l in open(sim + "/eng.out") if l.startswith("state ")]
    view = json.loads(st[-1][6:])["view"] if st else {}
    check("core:page over HTTP", code == 200 and view.get("page") == "more", view)

    code, _ = api("POST", "/api/notify", {"title": "Build failed", "text": "main"})
    check("POST /api/notify", code == 200, code)
    code, _ = api("DELETE", "/api/keys/build")
    code2, keys = api("GET", "/api/keys")
    check("DELETE /api/keys/build", code == 200 and keys == {}, keys)
    code, _ = api("POST", "/api/keys/99/press")
    check("bad key: 400", code == 400, code)
finally:
    eng.terminate(); dev.terminate()

print(f"{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
