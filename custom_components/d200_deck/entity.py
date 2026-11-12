"""The device every D200 Deck entity belongs to."""

from __future__ import annotations

from homeassistant.core import callback
from homeassistant.helpers.device_registry import DeviceInfo
from homeassistant.helpers.dispatcher import async_dispatcher_connect
from homeassistant.helpers.entity import Entity

from .const import DOMAIN


class DeckEntity(Entity):
    _attr_has_entity_name = True
    _attr_should_poll = False

    def __init__(self, bridge, key: str) -> None:
        self.bridge = bridge
        entry = bridge.entry
        self._attr_unique_id = f"{entry.entry_id}_{key}"
        self._attr_translation_key = key
        self._attr_device_info = DeviceInfo(
            identifiers={(DOMAIN, entry.entry_id)},
            name="D200 Deck",
            manufacturer="Ulanzi",
            model="D200 with D200 Deck",
            configuration_url=f"{bridge.api.base}/api/openapi.json" if bridge.api else None,
        )

    async def async_added_to_hass(self) -> None:
        self.async_on_remove(async_dispatcher_connect(self.hass, self.bridge.signal, self._update))

    @callback
    def _update(self, data) -> None:
        self.async_write_ha_state()
