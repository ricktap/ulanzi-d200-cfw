"""Minimal MQTT 3.1.1 broker for tests: QoS 0/1 in, QoS 0 out, retained, + and # wildcards."""
import socket, struct, threading

def match(flt, topic):
    f, t = flt.split("/"), topic.split("/")
    for i, p in enumerate(f):
        if p == "#":
            return True
        if i >= len(t) or (p != "+" and p != t[i]):
            return False
    return len(f) == len(t)

def enc_len(n):
    out = bytearray()
    while True:
        b = n % 128; n //= 128
        out.append(b | (0x80 if n else 0))
        if not n: return bytes(out)

def s16(b): return struct.pack(">H", len(b)) + b

class Broker:
    def __init__(self, port):
        self.retained, self.clients, self.log, self.lock = {}, [], [], threading.Lock()
        self.srv = socket.socket(); self.srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.srv.bind(("127.0.0.1", port)); self.srv.listen()
        threading.Thread(target=self.accept, daemon=True).start()

    def accept(self):
        while True:
            c, _ = self.srv.accept()
            cl = {"sock": c, "subs": [], "wlock": threading.Lock()}
            with self.lock: self.clients.append(cl)
            threading.Thread(target=self.serve, args=(cl,), daemon=True).start()

    def send(self, cl, data):
        try:
            with cl["wlock"]: cl["sock"].sendall(data)
        except OSError: pass

    def deliver(self, cl, topic, payload, retain=False):
        body = s16(topic.encode()) + payload
        self.send(cl, bytes([0x30 | (1 if retain else 0)]) + enc_len(len(body)) + body)

    def publish(self, topic, payload, retain=False, origin="test"):
        if isinstance(payload, str): payload = payload.encode()
        with self.lock:
            self.log.append((origin, topic, payload, retain))
            if retain:
                if payload: self.retained[topic] = payload
                else: self.retained.pop(topic, None)
            targets = [c for c in self.clients if any(match(f, topic) for f in c["subs"])]
        for c in targets: self.deliver(c, topic, payload)

    def serve(self, cl):
        s = cl["sock"]; f = s.makefile("rb")
        try:
            while True:
                h = f.read(1)
                if not h: break
                mult, n = 1, 0
                while True:
                    b = f.read(1)[0]; n += (b & 127) * mult; mult *= 128
                    if not b & 128: break
                body = f.read(n); typ, flags = h[0] >> 4, h[0] & 15
                if typ == 1: self.send(cl, b"\x20\x02\x00\x00")
                elif typ == 3:
                    tl = struct.unpack(">H", body[:2])[0]; topic = body[2:2 + tl].decode(); i = 2 + tl
                    qos = (flags >> 1) & 3
                    if qos: pid = body[i:i + 2]; i += 2; self.send(cl, b"\x40\x02" + pid)
                    self.publish(topic, body[i:], bool(flags & 1), origin="client")
                elif typ == 8:
                    pid, i, granted, new = body[:2], 2, b"", []
                    while i < len(body):
                        tl = struct.unpack(">H", body[i:i + 2])[0]; flt = body[i + 2:i + 2 + tl].decode(); i += 3 + tl
                        new.append(flt); granted += b"\x00"
                    cl["subs"] += new
                    self.send(cl, bytes([0x90]) + enc_len(2 + len(granted)) + pid + granted)
                    with self.lock: ret = list(self.retained.items())
                    for t, p in ret:
                        if any(match(flt, t) for flt in new): self.deliver(cl, t, p, retain=True)
                elif typ == 12: self.send(cl, b"\xd0\x00")
                elif typ == 14: break
        except Exception as e:
            pass
        finally:
            with self.lock:
                if cl in self.clients: self.clients.remove(cl)
            s.close()

    def got(self, topic, origin="client"):
        with self.lock: return [p for (o, t, p, r) in self.log if t == topic and o == origin]
