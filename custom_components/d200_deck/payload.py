"""What a Home Assistant entity looks like on a D200 key.

Pure functions (no Home Assistant imports), so they can be tested on their own.
The payload is what D200 Deck reads on `<base>/ha/<id>/set` (docs/home-assistant.md):
label, kind, icon, on, color, value, progress, background.
"""

from __future__ import annotations

from typing import Any

# The icon the deck picks per domain (`kind`, an icon of the plugin com.deckling.ha).
KINDS = {
    "light": "light",
    "switch": "switch",
    "input_boolean": "switch",
    "fan": "switch",
    "sensor": "sensor",
    "binary_sensor": "sensor",
    "climate": "climate",
    "water_heater": "climate",
    "lock": "lock",
    "cover": "cover",
    "scene": "scene",
    "script": "scene",
    "vacuum": "vacuum",
    "automation": "automation",
    "person": "presence",
    "device_tracker": "presence",
}

# States that count as "on" for the toggle role.
ON_STATES = {"on", "open", "opening", "unlocked", "home", "playing", "cleaning", "heat", "cool", "heat_cool", "auto", "dry", "fan_only"}
UNAVAILABLE = {"unavailable", "unknown"}


def _num(v: Any) -> float | None:
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def _fmt(n: float) -> str:
    return str(int(n)) if float(n).is_integer() else f"{n:.1f}"


def _hex(rgb: Any) -> str | None:
    try:
        r, g, b = (max(0, min(255, int(x))) for x in list(rgb)[:3])
    except (TypeError, ValueError):
        return None
    return f"#{r:02X}{g:02X}{b:02X}"


def key_id(entity_id: str) -> str:
    """The id on the deck: the entity id (letters, digits, `_`, `-`, `.`)."""
    return "".join(c if c.isalnum() or c in "_-." else "_" for c in entity_id)


def entity_payload(entity_id: str, state: str | None, attrs: dict[str, Any] | None, label: str | None = None) -> dict[str, Any]:
    """The key for one entity state. `state` None: the entity is gone."""
    attrs = attrs or {}
    domain = entity_id.split(".", 1)[0]
    out: dict[str, Any] = {"label": label or attrs.get("friendly_name") or entity_id}
    kind = KINDS.get(domain, "entity")
    if domain == "sensor" and attrs.get("device_class") in ("energy", "power"):
        kind = "energy"
    if domain == "lock":
        kind = "unlock" if state == "unlocked" else "lock"
    out["kind"] = kind

    if state is None or state in UNAVAILABLE:
        out["value"] = "n/a"
        return out

    if domain in ("light", "switch", "input_boolean", "fan", "automation", "cover", "lock", "media_player", "vacuum", "climate", "binary_sensor", "person", "device_tracker"):
        out["on"] = state in ON_STATES

    if domain == "light":
        if state == "on":
            col = _hex(attrs.get("rgb_color"))
            if col:
                out["color"] = col
            b = _num(attrs.get("brightness"))
            if b is not None:
                pct = round(b / 255 * 100)
                out["progress"] = pct
                out["value"] = f"{pct}%"
            else:
                out["value"] = "on"
        else:
            out["value"] = "off"
    elif domain == "sensor":
        n = _num(state)
        unit = attrs.get("unit_of_measurement") or ""
        out["value"] = f"{_fmt(n)}{unit if unit in ('%', '°C', '°F') else (' ' + unit if unit else '')}" if n is not None else state
        if unit == "%" and n is not None:
            out["progress"] = max(0, min(100, n))
    elif domain == "climate":
        cur = _num(attrs.get("current_temperature"))
        target = _num(attrs.get("temperature"))
        out["value"] = f"{_fmt(cur)}°" if cur is not None else state
        if target is not None:
            out["label"] = f"{out['label']} → {_fmt(target)}°"
    elif domain == "cover":
        pos = _num(attrs.get("current_position"))
        if pos is not None:
            out["progress"] = max(0, min(100, pos))
            out["value"] = f"{_fmt(pos)}%"
        else:
            out["value"] = state
    elif domain == "media_player":
        out["value"] = attrs.get("media_title") or state
        vol = _num(attrs.get("volume_level"))
        if vol is not None:
            out["progress"] = round(vol * 100)
    elif domain == "person" or domain == "device_tracker":
        out["value"] = {"home": "home", "not_home": "away"}.get(state, state)
    elif domain in ("scene", "script", "button", "input_button"):
        pass  # nothing to show but the label
    else:
        out["value"] = state
    return out


# What a short press does when the user did not choose otherwise: (domain, service).
def default_action(entity_id: str, state: str | None) -> tuple[str, str] | None:
    domain = entity_id.split(".", 1)[0]
    if domain in ("light", "switch", "input_boolean", "fan", "automation", "cover", "media_player", "climate", "humidifier", "siren"):
        return (domain, "toggle")
    if domain in ("scene", "script"):
        return (domain, "turn_on")
    if domain in ("button", "input_button"):
        return (domain, "press")
    if domain == "lock":
        return ("lock", "unlock" if state == "locked" else "lock")
    if domain == "vacuum":
        return ("vacuum", "return_to_base" if state == "cleaning" else "start")
    return None
