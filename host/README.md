# host/ (legacy)

The first prototype: a Python client (`d200.py`, needs Pillow) and a JSON-driven deck
(`deck.py`, `deck.example.json`). It uses the old v1 config format and is **not** compatible with
the D200 Deck app. It's kept as a minimal reference client for the protocol in
[../docs/protocol.md](../docs/protocol.md).

```sh
pip3 install Pillow
python3 host/deck.py host/deck.example.json
```
