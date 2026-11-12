"""Whether a computer has the deck (MQTT availability), or the Web API answers."""

from __future__ import annotations

from homeassistant.components.binary_sensor import BinarySensorDeviceClass, BinarySensorEntity
from homeassistant.const import EntityCategory

from .entity import DeckEntity


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    async_add_entities([Connected(entry.runtime_data)])


class Connected(DeckEntity, BinarySensorEntity):
    _attr_device_class = BinarySensorDeviceClass.CONNECTIVITY
    _attr_entity_category = EntityCategory.DIAGNOSTIC

    def __init__(self, bridge) -> None:
        super().__init__(bridge, "connected")

    @property
    def is_on(self) -> bool | None:
        return self.bridge.connected
