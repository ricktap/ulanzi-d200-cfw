"""Key events of the deck: every press, long press and double press on any key."""

from __future__ import annotations

from homeassistant.components.event import EventEntity
from homeassistant.core import callback

from .entity import DeckEntity

EVENT_TYPES = ["press", "long_press", "double_press"]


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    async_add_entities([KeyEventEntity(entry.runtime_data)])


class KeyEventEntity(DeckEntity, EventEntity):
    _attr_event_types = EVENT_TYPES

    def __init__(self, bridge) -> None:
        super().__init__(bridge, "key_event")

    @callback
    def _update(self, data) -> None:
        if not data or data.get("event") not in EVENT_TYPES:
            return
        attrs = {k: data[k] for k in ("key", "profile", "page") if k in data}
        self._trigger_event(data["event"], attrs)
        self.async_write_ha_state()
