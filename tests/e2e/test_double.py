"""Double press: two quick presses run `double` and not `press`; a single press runs `press`
after the wait; keys without `double` answer at once; a long second press counts as press
and long press. Real key events through the simulated D200.

Run through tests/e2e/run.sh. Args: <repo root> <work dir>.
"""
import json, os, shutil, struct, subprocess, sys, time
P, S = sys.argv[1], sys.argv[2]
CODES = [29, 15, 14, 13, 12, 11, 10, 9, 8, 7, 34, 33, 31, 30]

sim = S + "/simdouble"; shutil.rmtree(sim, ignore_errors=True); os.makedirs(sim + "/cfg")
open(sim + "/fb", "wb").write(bytes(720 * 1280 * 4)); os.mkfifo(sim + "/kbd")
log = sim + "/events.log"
def cmd(tag):
    return [{"action": "com.deckling.core:command", "params": {"command": f"echo {tag} >> {log}"}}]
cfg = {
    "version": 6,
    "plugins": {"core": {"config": {"follow_active_app": False, "screensaver": {"enabled": False}, "double_press_ms": 300, "long_press_ms": 500}}},
    "profiles": [{"name": "Default", "apps": [], "pages": {"main": {
        "0": {"states": [{"label": "Two", "on": {"press": cmd("k0-press"), "double": cmd("k0-double"), "long": cmd("k0-long")}}]},
        "1": {"states": [{"label": "One", "on": {"press": cmd("k1-press")}}]},
    }}}],
}
json.dump(cfg, open(sim + "/cfg/deck.json", "w"))

dev = subprocess.Popen([os.environ["D200FW_HOST_BIN"], "--no-kill", "--port", "7395",
                        "--fb", sim + "/fb", "--input", sim + "/kbd"], stderr=open(sim + "/devlog", "w"))
kbd = open(sim + "/kbd", "wb", buffering=0)
env = dict(os.environ, D200_DIRECT="1", D200_PORT="7395", D200_MEDIA_FILE=sim + "/no-player.txt")
eng = subprocess.Popen([os.environ.get("D200_HEADLESS", P + "/app/src-tauri/target/debug/examples/headless"), sim + "/cfg/deck.json"], env=env,
                       stdin=subprocess.PIPE, stdout=open(sim + "/eng.out", "w"), stderr=subprocess.STDOUT, text=True)

def key(k, down): kbd.write(struct.pack("<IIHHi", 0, 0, 1, CODES[k], 1 if down else 0))
def tap(k, hold=0.06): key(k, 1); time.sleep(hold); key(k, 0)
def events():
    try: return [l.strip() for l in open(log)]
    except FileNotFoundError: return []
def clear():
    if os.path.exists(log): os.remove(log)

results = []
def check(name, ok, extra=""):
    results.append(ok); print(("PASS " if ok else "FAIL ") + name, extra)

time.sleep(4)
try:
    # two quick presses: double, no press
    clear(); tap(0); time.sleep(0.1); tap(0); time.sleep(1.0)
    check("double press runs double only", events() == ["k0-double"], events())

    # one press: press, after the wait
    clear(); tap(0); time.sleep(0.12)
    early = events()
    time.sleep(0.9)
    check("single press waits, then runs press", early == [] and events() == ["k0-press"], (early, events()))

    # a key without double actions answers at once
    clear(); tap(1); time.sleep(0.15)
    check("key without double: press at once", events() == ["k1-press"], events())

    # long press on its own
    clear(); tap(0, hold=0.8); time.sleep(0.8)
    check("long press still works", events() == ["k0-long"], events())

    # a quick press, then a long second press: press, then long
    clear(); tap(0); time.sleep(0.1); tap(0, hold=0.8); time.sleep(0.8)
    check("press then long second press", events() == ["k0-press", "k0-long"], events())

    # two presses too far apart: two presses
    clear(); tap(0); time.sleep(0.55); tap(0); time.sleep(0.9)
    check("slow presses are two presses", events() == ["k0-press", "k0-press"], events())
finally:
    eng.terminate(); dev.terminate()

print(f"{sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
