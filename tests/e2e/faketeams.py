"""Fake Microsoft Teams third-party app API (WebSocket, protocol 2.0.0) for tests."""
import base64, hashlib, json, socket, struct, threading
from urllib.parse import urlparse, parse_qs

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"

class FakeTeams:
    def __init__(self, port):
        self.state = {"isInMeeting": True, "isMuted": False, "isVideoOn": False, "isHandRaised": False}
        self.actions, self.queries, self.conns = [], [], []
        self.srv = socket.socket(); self.srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.srv.bind(("127.0.0.1", port)); self.srv.listen()
        threading.Thread(target=self.accept, daemon=True).start()

    def accept(self):
        while True:
            c, _ = self.srv.accept()
            threading.Thread(target=self.serve, args=(c,), daemon=True).start()

    def send(self, c, obj):
        data = json.dumps(obj).encode()
        hdr = bytes([0x81]) + (bytes([len(data)]) if len(data) < 126 else bytes([126]) + struct.pack(">H", len(data)))
        try: c.sendall(hdr + data)
        except OSError: pass

    def update(self, c):
        self.send(c, {"meetingUpdate": {"meetingState": dict(self.state),
                      "meetingPermissions": {"canToggleMute": True, "canToggleVideo": True, "canPair": True}}})

    def drop_all(self):
        for c in list(self.conns):
            try: c.shutdown(socket.SHUT_RDWR); c.close()
            except OSError: pass
        self.conns.clear()

    def serve(self, c):
        f = c.makefile("rb")
        req = b""
        while not req.endswith(b"\r\n\r\n"):
            ch = f.read(1)
            if not ch: return
            req += ch
        lines = req.decode().split("\r\n")
        q = parse_qs(urlparse(lines[0].split(" ")[1]).query)
        self.queries.append({k: v[0] for k, v in q.items()})
        key = [l.split(": ", 1)[1] for l in lines if l.lower().startswith("sec-websocket-key")][0]
        acc = base64.b64encode(hashlib.sha1((key + GUID).encode()).digest()).decode()
        c.sendall(f"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {acc}\r\n\r\n".encode())
        self.conns.append(c)
        paired = "token" in q
        self.update(c)
        try:
            while True:
                b0, b1 = f.read(2)
                op, n = b0 & 15, b1 & 127
                if n == 126: n = struct.unpack(">H", f.read(2))[0]
                elif n == 127: n = struct.unpack(">Q", f.read(8))[0]
                mask = f.read(4) if b1 & 128 else b"\0\0\0\0"
                data = bytes(x ^ mask[i % 4] for i, x in enumerate(f.read(n)))
                if op == 8: break
                if op == 9: c.sendall(bytes([0x8A, len(data)]) + data); continue
                if op != 1: continue
                msg = json.loads(data); self.actions.append(msg)
                if not paired:
                    self.send(c, {"tokenRefresh": "tok-123"}); paired = True
                a = msg.get("action")
                if a == "toggle-mute": self.state["isMuted"] = not self.state["isMuted"]
                if a == "toggle-video": self.state["isVideoOn"] = not self.state["isVideoOn"]
                if a == "toggle-hand": self.state["isHandRaised"] = not self.state["isHandRaised"]
                self.send(c, {"response": "Success", "requestId": msg.get("requestId")})
                self.update(c)
        except Exception:
            pass
        finally:
            c.close()
