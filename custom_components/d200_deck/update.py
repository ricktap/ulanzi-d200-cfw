"""Web API only: is there a newer D200 Deck? (Installing stays in the app.)"""

from __future__ import annotations

from homeassistant.components.update import UpdateEntity

from .entity import StateEntity, web_only

RELEASES = "https://github.com/ricktap/ulanzi-d200-cfw/releases/tag/v"


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if web_only(entry):  # over MQTT the deck's own device has it
        async_add_entities([AppUpdate(entry.runtime_data)])


class AppUpdate(StateEntity, UpdateEntity):
    _attr_title = "D200 Deck"

    def __init__(self, bridge) -> None:
        super().__init__(bridge, "app_update", "version", diagnostic=True)

    @property
    def installed_version(self) -> str | None:
        return self.get("version")

    @property
    def latest_version(self) -> str | None:
        return self.get("latest") or self.get("version")

    @property
    def release_url(self) -> str | None:
        v = self.latest_version
        return RELEASES + v if v else None
