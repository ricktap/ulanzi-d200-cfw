"""Web API only: profile and page."""

from __future__ import annotations

from homeassistant.components.select import SelectEntity

from .entity import StateEntity, web_only


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if not web_only(entry):
        return  # over MQTT the deck's own device has these
    b = entry.runtime_data
    async_add_entities([
        Choice(b, "profile", "profile", "profiles", "core:profile", icon="mdi:view-grid"),
        Choice(b, "page", "page", "pages", "core:page", icon="mdi:book-open-page-variant"),
    ])


class Choice(StateEntity, SelectEntity):
    def __init__(self, bridge, key, field, options_field, action, **kw) -> None:
        super().__init__(bridge, key, field, **kw)
        self.options_field, self.action = options_field, action

    @property
    def options(self) -> list[str]:
        return list(self.get(self.options_field) or [])

    @property
    def current_option(self) -> str | None:
        v = self.get()
        return v if v in self.options else None

    async def async_select_option(self, option: str) -> None:
        await self.bridge.run(self.action, {self.field: option})
