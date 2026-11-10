"""Host-side client for d200fw running on an Ulanzi D200 / D200H.

    from d200 import D200
    deck = D200()                       # runs `adb forward` and connects
    deck.set_text(0, "Hello", bg="#1e3a8a")
    deck.on_key(lambda key, down: print(key, down))
    deck.wait()

Keys are numbered in reading order: 0-4 top row, 5-9 middle row,
10-12 bottom row, 13 the wide window bottom right (456x196).
Needs Pillow (pip install Pillow).
"""

from __future__ import annotations

import queue
import shutil
import socket
import subprocess
import threading
from typing import Callable

from PIL import Image, ImageDraw, ImageFont

NUM_KEYS = 14

_FONT_CANDIDATES = [
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/Library/Fonts/Arial.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "DejaVuSans-Bold.ttf",
]


def load_font(size: int) -> ImageFont.ImageFont:
    for path in _FONT_CANDIDATES:
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            continue
    return ImageFont.load_default(size=size)


class D200Error(RuntimeError):
    pass


class D200:
    def __init__(self, host: str = "127.0.0.1", port: int = 7300, adb_forward: bool = True, timeout: float = 5.0):
        if adb_forward and host in ("127.0.0.1", "localhost"):
            if not shutil.which("adb"):
                raise D200Error("adb not found (macOS: brew install android-platform-tools)")
            subprocess.run(["adb", "forward", f"tcp:{port}", f"tcp:{port}"], check=True, capture_output=True)
        self._sock = socket.create_connection((host, port), timeout=timeout)
        self._sock.settimeout(None)
        self._sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self._rfile = self._sock.makefile("rb")
        self._lock = threading.Lock()
        self._replies: queue.Queue[str | None] = queue.Queue()
        self._callbacks: list[Callable[[int, bool], None]] = []
        self._events: queue.Queue[tuple[int, bool] | None] = queue.Queue()
        self._closed = threading.Event()
        self._timeout = timeout
        threading.Thread(target=self._reader, daemon=True).start()
        threading.Thread(target=self._dispatch, daemon=True).start()

        pong = self._cmd("PING")
        if not pong.startswith("PONG d200fw"):
            raise D200Error(f"not a d200fw device: {pong!r}")
        self.version = pong.split()[-1]
        self.rects = self._layout()

    # ------------------------------------------------------------ protocol

    def _reader(self) -> None:
        try:
            for raw in self._rfile:
                line = raw.decode("utf-8", "replace").strip()
                if line.startswith("KEY "):
                    _, key, down = line.split()
                    self._events.put((int(key), down == "1"))
                elif line:
                    self._replies.put(line)
        finally:
            self._closed.set()
            self._replies.put(None)
            self._events.put(None)

    def _dispatch(self) -> None:
        # Separate thread so callbacks can send commands (the reader must stay free
        # to deliver their replies).
        while (ev := self._events.get()) is not None:
            for cb in list(self._callbacks):
                try:
                    cb(*ev)
                except Exception as e:
                    print(f"d200: key callback failed: {e!r}")

    def _next_reply(self) -> str:
        try:
            r = self._replies.get(timeout=self._timeout)
        except queue.Empty:
            raise D200Error("device did not answer") from None
        if r is None:
            raise D200Error("connection to device closed")
        return r

    def _cmd(self, line: str, payload: bytes = b"") -> str:
        with self._lock:
            self._sock.sendall(line.encode() + b"\n" + payload)
            reply = self._next_reply()
        if reply.startswith("ERR"):
            raise D200Error(reply)
        return reply

    def _layout(self) -> dict[int, tuple[int, int, int, int]]:
        rects = {}
        with self._lock:
            self._sock.sendall(b"LAYOUT\n")
            while True:
                r = self._next_reply()
                if r == "OK":
                    break
                _, i, x, y, w, h = r.split()
                rects[int(i)] = (int(x), int(y), int(w), int(h))
        return rects

    # ------------------------------------------------------------ API

    def key_size(self, key: int) -> tuple[int, int]:
        _, _, w, h = self.rects[key]
        return w, h

    def set_image(self, key: int, img: Image.Image) -> None:
        """Show a PIL image on a key. It is scaled to cover the key and centre-cropped."""
        w, h = self.key_size(key)
        img = img.convert("RGB")
        if img.size != (w, h):
            scale = max(w / img.width, h / img.height)
            img = img.resize((max(w, round(img.width * scale)), max(h, round(img.height * scale))), Image.LANCZOS)
            left, top = (img.width - w) // 2, (img.height - h) // 2
            img = img.crop((left, top, left + w, top + h))
        data = img.tobytes()
        self._cmd(f"IMG {key} {len(data)}", data)

    def render_text(self, key: int, text: str, bg: str = "#202430", fg: str = "white",
                    size: int | None = None, icon: Image.Image | None = None) -> Image.Image:
        """Render a key face: optional icon on top, text (may contain newlines) below."""
        w, h = self.key_size(key)
        img = Image.new("RGB", (w, h), bg)
        d = ImageDraw.Draw(img)
        text_top = 0
        if icon is not None:
            ic = icon.convert("RGBA")
            ic.thumbnail((h * 0.55, h * 0.55))
            img.paste(ic, ((w - ic.width) // 2, int(h * 0.08)), ic)
            text_top = int(h * 0.08) + ic.height
        if text:
            size = size or (30 if icon is not None else 40)
            font = load_font(size)
            while size > 12:
                box = d.multiline_textbbox((0, 0), text, font=font, align="center", spacing=4)
                if box[2] - box[0] <= w - 16 and box[3] - box[1] <= h - text_top - 12:
                    break
                size -= 2
                font = load_font(size)
            box = d.multiline_textbbox((0, 0), text, font=font, align="center", spacing=4)
            tw, th = box[2] - box[0], box[3] - box[1]
            x = (w - tw) // 2 - box[0]
            y = text_top + (h - text_top - th) // 2 - box[1]
            d.multiline_text((x, y), text, font=font, fill=fg, align="center", spacing=4)
        return img

    def set_text(self, key: int, text: str, **kw) -> None:
        self.set_image(key, self.render_text(key, text, **kw))

    def fill(self, key: int, color: str) -> None:
        self._cmd(f"FILL {key} {_hex(color)}")

    def clear(self, color: str = "#000000") -> None:
        self._cmd(f"CLEAR {_hex(color)}")

    def blit(self, x: int, y: int, img: Image.Image) -> None:
        """Draw anywhere on the 1280x720 landscape screen (including between keys)."""
        img = img.convert("RGB")
        self._cmd(f"BLIT {x} {y} {img.width} {img.height}", img.tobytes())

    def brightness(self, value: int) -> None:
        self._cmd(f"BRIGHT {max(0, min(255, int(value)))}")

    def info(self) -> str:
        return self._cmd("INFO")

    def on_key(self, callback: Callable[[int, bool], None]) -> None:
        """callback(key, pressed) runs on a background thread for every press and release."""
        self._callbacks.append(callback)

    def wait(self, timeout: float | None = None) -> bool:
        """Block until the connection closes. Returns True if it did."""
        return self._closed.wait(timeout)

    @property
    def connected(self) -> bool:
        return not self._closed.is_set()

    def close(self) -> None:
        try:
            self._sock.sendall(b"BYE\n")
        except OSError:
            pass
        self._sock.close()


def _hex(color: str) -> str:
    from PIL import ImageColor

    r, g, b = ImageColor.getrgb(color)[:3]
    return f"{r:02x}{g:02x}{b:02x}"
