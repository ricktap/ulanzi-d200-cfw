"""The deck's Web API (plugin Web API in D200 Deck), from Home Assistant."""

from __future__ import annotations

import logging
from typing import Any

import aiohttp

from homeassistant.core import HomeAssistant
from homeassistant.helpers.aiohttp_client import async_get_clientsession

_LOGGER = logging.getLogger(__name__)
TIMEOUT = aiohttp.ClientTimeout(total=10)


class WebError(Exception):
    """The deck did not answer, or refused the token (`auth` is True then)."""

    def __init__(self, msg: str, auth: bool = False) -> None:
        super().__init__(msg)
        self.auth = auth


class DeckApi:
    def __init__(self, hass: HomeAssistant, host: str, port: int, token: str) -> None:
        self.hass = hass
        self.base = f"http://{host}:{port}"
        self.token = token

    async def call(self, method: str, path: str, body: Any = None) -> Any:
        session = async_get_clientsession(self.hass)
        try:
            async with session.request(
                method, self.base + path, json=body, headers={"Authorization": f"Bearer {self.token}"}, timeout=TIMEOUT
            ) as r:
                if r.status == 401:
                    raise WebError("the deck refused the token", auth=True)
                data = await r.json(content_type=None)
                if r.status >= 400:
                    raise WebError(f"{method} {path}: {r.status} {data}")
                return data
        except (aiohttp.ClientError, TimeoutError) as err:
            raise WebError(f"{self.base}: {err}") from err

    async def info(self) -> dict[str, Any]:
        return await self.call("GET", "/api")

    async def put_key(self, kid: str, payload: dict[str, Any] | None) -> None:
        if payload is None:
            try:
                await self.call("DELETE", f"/api/keys/{kid}")
            except WebError as err:
                if "404" not in str(err):
                    raise
        else:
            await self.call("PUT", f"/api/keys/{kid}", payload)

    async def notify(self, msg: dict[str, Any]) -> None:
        await self.call("POST", "/api/notify", msg)

    async def action(self, action: str, params: dict[str, Any] | None = None) -> None:
        await self.call("POST", f"/api/actions/{action}", {"params": params or {}})

    async def mqtt(self) -> dict[str, Any]:
        return await self.call("GET", "/api/mqtt")

    async def set_mqtt(self, settings: dict[str, Any]) -> None:
        await self.call("PUT", "/api/mqtt", settings)

    async def state(self) -> dict[str, Any]:
        return await self.call("GET", "/api/state")

    async def press(self, key: int, event: str) -> None:
        await self.call("POST", f"/api/keys/{key}/press", {"event": event})

    async def subscribe(self, url: str, ttl: int, keys: bool = False) -> None:
        # keys: every key event and every state change too (when MQTT does not bring them)
        events = ["presses", "keys", "state"] if keys else ["presses"]
        await self.call("POST", "/api/subscriptions", {"url": url, "ttl": ttl, "events": events})

    async def unsubscribe(self, url: str) -> None:
        await self.call("DELETE", "/api/subscriptions", {"url": url})
