"""The device the D200 Deck entities belong to.

Over MQTT the deck announces a device itself, "Ulanzi D200", with every control and sensor
(MQTT discovery). The integration joins that device and adds only the key events. Over the
Web API alone there is no such device; the integration makes it, with the same entities, read
from the deck's state (`GET /api/state`, pushed to the webhook when it changes).
"""

from __future__ import annotations

from typing import Any

from homeassistant.const import EntityCategory
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


def web_only(entry) -> bool:
    return not entry.runtime_data.use_mqtt


class StateEntity(DeckEntity):
    """Web API only: an entity that shows a field of the deck's state."""

    def __init__(self, bridge, key: str, field: str = "", icon: str | None = None, diagnostic: bool = False, config: bool = False) -> None:
        super().__init__(bridge, key)
        self.field = field or key
        if icon:
            self._attr_icon = icon
        if diagnostic:
            self._attr_entity_category = EntityCategory.DIAGNOSTIC
        elif config:
            self._attr_entity_category = EntityCategory.CONFIG

    def get(self, path: str | None = None) -> Any:
        v: Any = self.bridge.state
        for part in (path or self.field).split("."):
            v = v.get(part) if isinstance(v, dict) else None
        return v

    @property
    def available(self) -> bool:
        return self.bridge.connected is True
