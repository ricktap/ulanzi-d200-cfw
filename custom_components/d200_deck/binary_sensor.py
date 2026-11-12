"""Web API only: computer online, D200 plugged in, in a Teams meeting."""

from __future__ import annotations

from homeassistant.components.binary_sensor import BinarySensorDeviceClass, BinarySensorEntity
from homeassistant.const import EntityCategory

from .entity import DeckEntity, StateEntity, web_only


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if not web_only(entry):
        return  # over MQTT the deck's own device has these
    b = entry.runtime_data
    async_add_entities([
        Connected(b),
        Flag(b, "usb", device_class=BinarySensorDeviceClass.PLUG, diagnostic=True),
        Flag(b, "teams_meeting", "teams.in_meeting", icon="mdi:video"),
    ])


class Connected(DeckEntity, BinarySensorEntity):
    """The deck answers (checked every minute)."""

    _attr_device_class = BinarySensorDeviceClass.CONNECTIVITY
    _attr_entity_category = EntityCategory.DIAGNOSTIC

    def __init__(self, bridge) -> None:
        super().__init__(bridge, "connected")

    @property
    def is_on(self) -> bool | None:
        return self.bridge.connected


class Flag(StateEntity, BinarySensorEntity):
    def __init__(self, bridge, key, field="", device_class=None, **kw) -> None:
        super().__init__(bridge, key, field, **kw)
        self._attr_device_class = device_class

    @property
    def is_on(self) -> bool | None:
        v = self.get()
        return None if v is None and self.field.startswith("teams.") and not self.get("teams") else bool(v)
