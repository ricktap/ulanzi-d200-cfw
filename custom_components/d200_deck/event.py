"""Web API only: one event entity per key (Key 0 … Key 12, Wide key), as the deck announces
them itself over MQTT. Each shows press, long press and double press with the profile and page."""

from __future__ import annotations

from homeassistant.components.event import EventDeviceClass, EventEntity
from homeassistant.core import callback

from .entity import DeckEntity, web_only

EVENT_TYPES = ["press", "long_press", "double_press"]
KEYS = 14


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if web_only(entry):  # over MQTT the deck's own device has these
        async_add_entities([KeyEvent(entry.runtime_data, k) for k in range(KEYS)])


class KeyEvent(DeckEntity, EventEntity):
    _attr_event_types = EVENT_TYPES
    _attr_device_class = EventDeviceClass.BUTTON

    def __init__(self, bridge, key: int) -> None:
        super().__init__(bridge, f"key_{key}")
        self.key = key
        self._attr_translation_key = None
        self._attr_name = "Wide key" if key == KEYS - 1 else f"Key {key}"

    @callback
    def _update(self, data) -> None:
        if not data or data.get("key") != self.key or data.get("event") not in EVENT_TYPES:
            return
        self._trigger_event(data["event"], {k: data[k] for k in ("profile", "page") if k in data})
        self.async_write_ha_state()
