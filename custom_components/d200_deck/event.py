"""Web API only (over MQTT the deck announces these itself): Key pressed, for every key, and one
event entity per key (Key 0 … Key 12, Wide key). Each shows press, long press and double press;
the attributes say which key, on which profile and page, with its label, icon and app."""

from __future__ import annotations

from homeassistant.components.event import EventDeviceClass, EventEntity
from homeassistant.core import callback

from .entity import DeckEntity, web_only

EVENT_TYPES = ["press", "long_press", "double_press"]
KEYS = 14


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if web_only(entry):  # over MQTT the deck's own device has these
        b = entry.runtime_data
        async_add_entities([KeyEvent(b, None)] + [KeyEvent(b, k) for k in range(KEYS)])


def attributes(data) -> dict:
    """As the deck sends them over MQTT: `icon` would become the entity's own icon."""
    a = {k: data.get(k) for k in ("key", "profile", "page", "app")}
    a["key_label"], a["key_icon"] = data.get("label"), data.get("icon")
    return a


class KeyEvent(DeckEntity, EventEntity):
    _attr_event_types = EVENT_TYPES
    _attr_device_class = EventDeviceClass.BUTTON

    def __init__(self, bridge, key: int | None) -> None:
        """key None: every key (Key pressed)."""
        super().__init__(bridge, "key_pressed" if key is None else f"key_{key}")
        self.key = key
        self._attr_translation_key = None
        self._attr_name = "Key pressed" if key is None else "Wide key" if key == KEYS - 1 else f"Key {key}"

    @callback
    def _update(self, data) -> None:
        if not data or data.get("event") not in EVENT_TYPES or self.key not in (None, data.get("key")):
            return
        self._trigger_event(data["event"], attributes(data))
        self.async_write_ha_state()
