"""Web API only: profile, page, theme and screensaver scene."""

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
        Choice(b, "theme", "theme", "themes", "core:theme", icon="mdi:palette"),
        Choice(b, "screensaver_scene", "scene", SCENES, "core:screensaver_scene", param="scene", icon="mdi:image-filter-hdr", config=True),
    ])


SCENES = ["auto", "snow", "pink", "beach", "leaves", "rain"]


class Choice(StateEntity, SelectEntity):
    def __init__(self, bridge, key, field, options_field, action, param: str | None = None, **kw) -> None:
        """options_field: a field of the state with the options, or the options themselves."""
        super().__init__(bridge, key, field, **kw)
        self.options_field, self.action, self.param = options_field, action, param or field

    @property
    def options(self) -> list[str]:
        if isinstance(self.options_field, list):
            return self.options_field
        return list(self.get(self.options_field) or [])

    @property
    def current_option(self) -> str | None:
        v = self.get()
        return v if v in self.options else None

    async def async_select_option(self, option: str) -> None:
        await self.bridge.run(self.action, {self.param: option})
