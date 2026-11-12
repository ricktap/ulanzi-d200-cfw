"""The device the D200 Deck entities belong to.

Over MQTT the deck already announces a device, "Ulanzi D200", with brightness, profile,
notifications, connected and computer. The integration joins that device and adds only what it
lacks (key events, page). Over the Web API alone there is no such device; the integration makes
it, with the connected, computer and profile sensors as well.
"""

from __future__ import annotations

from homeassistant.core import callback
from homeassistant.helpers.device_registry import DeviceInfo
from homeassistant.helpers.dispatcher import async_dispatcher_connect
from homeassistant.helpers.entity import Entity

from .const import DOMAIN


def device_identifiers(bridge) -> set[tuple[str, str]]:
    if bridge.use_mqtt:
        # the identifier the deck uses in its MQTT discovery (mqtt.rs: `<base>_deck`)
        return {("mqtt", f"{bridge.base.replace('/', '_')}_deck")}
    return {(DOMAIN, bridge.entry.entry_id)}


def device_info(bridge) -> DeviceInfo:
    return DeviceInfo(
        identifiers=device_identifiers(bridge),
        name="Ulanzi D200",
        manufacturer="Ulanzi",
        model="D200 (d200fw)",
    )


class DeckEntity(Entity):
    _attr_has_entity_name = True
    _attr_should_poll = False

    def __init__(self, bridge, key: str) -> None:
        self.bridge = bridge
        self._attr_unique_id = f"{bridge.entry.entry_id}_{key}"
        self._attr_translation_key = key
        self._attr_device_info = device_info(bridge)

    async def async_added_to_hass(self) -> None:
        self.async_on_remove(async_dispatcher_connect(self.hass, self.bridge.signal, self._update))

    @callback
    def _update(self, data) -> None:
        self.async_write_ha_state()
