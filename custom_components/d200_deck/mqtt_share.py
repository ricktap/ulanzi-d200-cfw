"""Hand Home Assistant's own MQTT broker to the deck (over the deck's Web API)."""

from __future__ import annotations

from typing import Any

from homeassistant.components.network import async_get_source_ip
from homeassistant.core import HomeAssistant

# Broker names that only mean something inside Home Assistant (the Mosquitto add-on, itself).
LOCAL_NAMES = {"core-mosquitto", "localhost", "127.0.0.1", "::1", "a0d7b954-mosquitto"}


def ha_broker(hass: HomeAssistant) -> dict[str, Any] | None:
    """The MQTT integration's broker settings, or None when MQTT is not set up."""
    entries = hass.config_entries.async_entries("mqtt")
    if not entries:
        return None
    d = entries[0].data
    if not d.get("broker"):
        return None
    return {
        "host": d["broker"],
        "port": int(d.get("port", 1883)),
        "username": d.get("username") or "",
        "password": d.get("password") or "",
        "tls": bool(d.get("certificate")),
    }


async def broker_for_deck(hass: HomeAssistant) -> dict[str, Any] | None:
    """The broker as the deck must reach it: a name only Home Assistant knows becomes its address."""
    b = ha_broker(hass)
    if b is None:
        return None
    if b["host"] in LOCAL_NAMES or b["host"].endswith("-mosquitto"):
        b["host"] = await async_get_source_ip(hass)
    return b
