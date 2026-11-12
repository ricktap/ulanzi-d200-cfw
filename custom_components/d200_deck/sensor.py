"""The deck's current profile and page, and the computer that has it."""

from __future__ import annotations

from homeassistant.components.sensor import SensorEntity
from homeassistant.const import EntityCategory

from .entity import DeckEntity


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    b = entry.runtime_data
    async_add_entities([Value(b, "profile"), Value(b, "page"), Value(b, "computer", diagnostic=True)])


class Value(DeckEntity, SensorEntity):
    def __init__(self, bridge, field: str, diagnostic: bool = False) -> None:
        super().__init__(bridge, field)
        self.field = field
        if diagnostic:
            self._attr_entity_category = EntityCategory.DIAGNOSTIC

    @property
    def native_value(self) -> str | None:
        return getattr(self.bridge, self.field)
