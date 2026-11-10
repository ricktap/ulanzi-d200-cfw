#!/usr/bin/env python3
"""Config-driven stream deck for d200fw.

    python3 deck.py deck.example.json

Config: {"brightness": 0-255, "pages": {"<name>": {"<key 0-13>": {...}}}}
The first page is shown at start. Per key:
    label   text (use \\n for line breaks)
    bg, fg  colours (CSS names or #rrggbb)
    icon    image file drawn above the label
    image   image file that fills the whole key (label ignored)
    run     shell command run on the computer when the key is pressed
    page    switch to this page when pressed
    clock   true: show the time (and date on the wide key 13)
Paths are relative to the config file. Reconnects when the device is replugged.
"""

from __future__ import annotations

import json
import subprocess
import sys
import threading
import time
from pathlib import Path

from PIL import Image

from d200 import D200, D200Error, NUM_KEYS

PRESSED_BG = "#ffffff"


class Deck:
    def __init__(self, config_path: Path):
        self.base = config_path.parent
        self.cfg = json.loads(config_path.read_text())
        self.pages: dict[str, dict] = self.cfg["pages"]
        self.page = next(iter(self.pages))
        self.dev: D200 | None = None
        self.lock = threading.Lock()

    def key_cfg(self, key: int) -> dict:
        return self.pages[self.page].get(str(key), {})

    def _img(self, name: str) -> Image.Image:
        return Image.open(self.base / name)

    def draw(self, key: int, pressed: bool = False) -> None:
        c = self.key_cfg(key)
        dev = self.dev
        bg = PRESSED_BG if pressed else c.get("bg", "#000000" if not c else "#202430")
        fg = "#000000" if pressed else c.get("fg", "white")
        if c.get("image") and not pressed:
            dev.set_image(key, self._img(c["image"]))
        elif c.get("clock"):
            now = time.localtime()
            text = time.strftime("%H:%M", now)
            if dev.key_size(key)[0] > 300:
                text += time.strftime("\n%a %d %b", now)
            dev.set_text(key, text, bg=bg, fg=fg, size=64)
        else:
            icon = self._img(c["icon"]) if c.get("icon") else None
            label = c.get("label", "").replace("\\n", "\n")
            dev.set_text(key, label, bg=bg, fg=fg, icon=icon)

    def draw_all(self) -> None:
        for k in range(NUM_KEYS):
            self.draw(k)

    def on_key(self, key: int, down: bool) -> None:
        c = self.key_cfg(key)
        if not c:
            return
        with self.lock:
            self.draw(key, pressed=down)
            if down:
                return
            if "page" in c and c["page"] in self.pages:
                self.page = c["page"]
                self.draw_all()
        if c.get("run"):
            print(f"key {key}: {c['run']}")
            subprocess.Popen(c["run"], shell=True, cwd=self.base)

    def tick(self) -> None:
        # Redraw clocks once a minute (on the minute).
        last = None
        while self.dev and self.dev.connected:
            minute = time.strftime("%H:%M")
            if minute != last:
                last = minute
                with self.lock:
                    for k in range(NUM_KEYS):
                        if self.key_cfg(k).get("clock"):
                            try:
                                self.draw(k)
                            except D200Error:
                                return
            time.sleep(1)

    def run(self) -> None:
        while True:
            try:
                self.dev = D200()
            except (OSError, D200Error, subprocess.CalledProcessError) as e:
                print(f"waiting for device: {e}")
                time.sleep(3)
                continue
            print(f"connected to d200fw {self.dev.version}")
            try:
                if "brightness" in self.cfg:
                    self.dev.brightness(self.cfg["brightness"])
                self.dev.clear("#000000")
                with self.lock:
                    self.draw_all()
                self.dev.on_key(self.on_key)
                threading.Thread(target=self.tick, daemon=True).start()
                self.dev.wait()
            except D200Error as e:
                print(f"device error: {e}")
            print("disconnected, retrying")
            time.sleep(2)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    try:
        Deck(Path(sys.argv[1]).resolve()).run()
    except KeyboardInterrupt:
        pass
