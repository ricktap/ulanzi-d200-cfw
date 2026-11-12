"""Web API only: computer, program in front, Teams presence, now playing, Pomodoro."""

from __future__ import annotations

from typing import Any

from homeassistant.components.sensor import SensorEntity

from .entity import StateEntity, web_only


async def async_setup_entry(hass, entry, async_add_entities) -> None:
    if not web_only(entry):
        return  # over MQTT the deck's own device has these
    b = entry.runtime_data
    async_add_entities([
        Value(b, "computer", icon="mdi:monitor", diagnostic=True),
        Value(b, "app", icon="mdi:application"),
        Value(b, "teams_presence", "teams.presence", icon="mdi:account-circle", attrs="teams"),
        NowPlaying(b, "now_playing", "media", icon="mdi:music", attrs="media"),
        Value(b, "pomodoro", "pomodoro.phase", icon="mdi:timer-outline", attrs="pomodoro"),
    ])


class Value(StateEntity, SensorEntity):
    def __init__(self, bridge, key, field="", attrs: str | None = None, **kw) -> None:
        super().__init__(bridge, key, field, **kw)
        self.attrs = attrs

    @property
    def native_value(self) -> Any:
        v = self.get()
        return v if v not in ("", None) else None

    @property
    def extra_state_attributes(self) -> dict[str, Any] | None:
        a = self.get(self.attrs) if self.attrs else None
        return dict(a) if isinstance(a, dict) else None


class NowPlaying(Value):
    @property
    def native_value(self) -> str:
        m = self.get() or {}
        return " – ".join(x for x in (m.get("title"), m.get("artist")) if x) or "idle"
