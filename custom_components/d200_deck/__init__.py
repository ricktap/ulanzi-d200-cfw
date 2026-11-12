"""D200 Deck: Home Assistant entities on the keys of a Ulanzi D200.

D200 Deck (the desktop app that drives the D200) is reached over MQTT, over its Web API
(plugin Web API, token), or both. The entities you pick appear on the deck as ids you can
put on any key, kept up to date; a press runs the entity's default action (toggle a light,
run a scene) and fires the event `d200_deck_press` for your own automations. Services send
notifications and describe custom keys.

MQTT (base `d200`): `d200/ha/<id>/set` retained, `d200/ha/<id>/action`, `d200/notify`,
`d200/remote/show`, `d200/availability`. On the deck these keys use the app Home Assistant.

Web API: `PUT /api/keys/<id>`, `POST /api/notify`, `POST /api/actions/...`; presses come
back to a Home Assistant webhook the integration registers with `POST /api/subscriptions`
(renewed every few minutes, when every key is also sent again). On the deck these keys use
the app Web key.
"""

from __future__ import annotations

from datetime import timedelta
import json
import logging
from typing import Any

from aiohttp import web
import voluptuous as vol

from homeassistant.components import mqtt, webhook
from homeassistant.config_entries import ConfigEntry
from homeassistant.const import ATTR_ENTITY_ID
from homeassistant.core import Event, EventStateChangedData, HomeAssistant, ServiceCall, callback
from homeassistant.exceptions import HomeAssistantError
from homeassistant.helpers import config_validation as cv
from homeassistant.helpers.event import async_track_state_change_event, async_track_time_interval
from homeassistant.helpers.network import NoURLAvailableError

from .const import (
    CONF_BASE_TOPIC,
    CONF_DEFAULT_ACTIONS,
    CONF_ENTITIES,
    CONF_HOST,
    CONF_PORT,
    CONF_TOKEN,
    CONF_TRANSPORT,
    CONF_WEBHOOK_ID,
    DEFAULT_BASE_TOPIC,
    DEFAULT_PORT,
    DOMAIN,
    EVENT_PRESS,
    SERVICE_CLEAR_KEY,
    SERVICE_NOTIFY,
    SERVICE_SET_KEY,
    SERVICE_SHOW_REMOTE,
    TRANSPORT_BOTH,
    TRANSPORT_MQTT,
    TRANSPORT_WEB,
)
from .payload import default_action, entity_payload, key_id
from .web import DeckApi, WebError

_LOGGER = logging.getLogger(__name__)

CONFIG_SCHEMA = cv.config_entry_only_config_schema(DOMAIN)

# Web API: the webhook subscription lives this long and is renewed (with every key) this often.
SUBSCRIPTION_TTL = 900
RESYNC = timedelta(minutes=5)

KEY_FIELDS = {
    vol.Optional("label"): cv.string,
    vol.Optional("kind"): cv.string,
    vol.Optional("icon"): cv.string,
    vol.Optional("value"): cv.string,
    vol.Optional("on"): cv.boolean,
    vol.Optional("color"): cv.string,
    vol.Optional("progress"): vol.All(vol.Coerce(float), vol.Range(min=0, max=100)),
    vol.Optional("background"): cv.string,
}
ID_RE = vol.Match(r"^[A-Za-z0-9_.\-]+$")


class Bridge:
    """One D200 Deck: publishes the chosen entities, answers presses."""

    def __init__(self, hass: HomeAssistant, entry: ConfigEntry) -> None:
        self.hass = hass
        self.entry = entry
        data = entry.data
        transport = data.get(CONF_TRANSPORT, TRANSPORT_MQTT)
        self.use_mqtt = transport in (TRANSPORT_MQTT, TRANSPORT_BOTH)
        self.use_web = transport in (TRANSPORT_WEB, TRANSPORT_BOTH)
        self.base: str = (data.get(CONF_BASE_TOPIC) or DEFAULT_BASE_TOPIC).strip("/")
        self.host: str | None = data.get(CONF_HOST)
        self.api = DeckApi(hass, data[CONF_HOST], data.get(CONF_PORT, DEFAULT_PORT), data[CONF_TOKEN]) if self.use_web else None
        self.webhook_id: str | None = data.get(CONF_WEBHOOK_ID)
        self.webhook_url: str | None = None
        self.entities: list[str] = list(entry.options.get(CONF_ENTITIES, []))
        self.default_actions: bool = entry.options.get(CONF_DEFAULT_ACTIONS, True)
        self._unsub: list[Any] = []

    def topic(self, rest: str) -> str:
        return f"{self.base}/{rest}"

    def matches(self, deck: str | None) -> bool:
        """For services: `base_topic` names a deck by its base topic or its host."""
        return not deck or deck.strip("/") in (self.base if self.use_mqtt else None, self.host)

    async def _web(self, what: str, coro) -> None:
        try:
            await coro
        except WebError as err:
            _LOGGER.warning("D200 Deck Web API (%s): %s", what, err)

    async def publish_key(self, kid: str, payload: dict[str, Any] | None) -> None:
        """Describe key `kid`; None removes it from the deck."""
        if self.use_mqtt:
            body = "" if payload is None else json.dumps(payload, ensure_ascii=False)
            await mqtt.async_publish(self.hass, self.topic(f"ha/{kid}/set"), body, qos=0, retain=True)
        if self.api:
            await self._web(f"key {kid}", self.api.put_key(kid, payload))

    async def notify(self, msg: dict[str, Any]) -> None:
        if self.use_mqtt:
            await mqtt.async_publish(self.hass, self.topic("notify"), json.dumps(msg, ensure_ascii=False))
        elif self.api:
            await self._web("notify", self.api.notify(msg))

    async def show_remote(self, show: bool) -> None:
        if self.use_mqtt:
            await mqtt.async_publish(self.hass, self.topic("remote/show"), "on" if show else "off")
        elif self.api:
            await self._web("remote page", self.api.action("com.deckling.ha:remote" if show else "core:home"))

    async def publish_entity(self, entity_id: str) -> None:
        st = self.hass.states.get(entity_id)
        await self.publish_key(
            key_id(entity_id),
            entity_payload(entity_id, st.state if st else None, dict(st.attributes) if st else None),
        )

    async def publish_all(self) -> None:
        for e in self.entities:
            await self.publish_entity(e)

    async def start(self) -> None:
        await self.publish_all()

        @callback
        def changed(event: Event[EventStateChangedData]) -> None:
            self.hass.async_create_task(self.publish_entity(event.data["entity_id"]))

        if self.entities:
            self._unsub.append(async_track_state_change_event(self.hass, self.entities, changed))

        if self.use_mqtt:
            await self._start_mqtt()
        if self.api:
            await self._start_web()

    async def _start_mqtt(self) -> None:
        @callback
        def action(msg: mqtt.ReceiveMessage) -> None:
            parts = msg.topic.split("/")
            if len(parts) < 3:
                return
            payload = msg.payload if isinstance(msg.payload, str) else msg.payload.decode()
            self.hass.async_create_task(self.press(parts[-2], payload.strip()))

        self._unsub.append(await mqtt.async_subscribe(self.hass, self.topic("ha/+/action"), action))

        @callback
        def availability(msg: mqtt.ReceiveMessage) -> None:
            # a computer took the D200: make sure it has every key (retained anyway, cheap)
            payload = msg.payload if isinstance(msg.payload, str) else msg.payload.decode()
            if payload.startswith("online"):
                self.hass.async_create_task(self.publish_all())

        self._unsub.append(await mqtt.async_subscribe(self.hass, self.topic("availability"), availability))

    async def _start_web(self) -> None:
        if self.webhook_id:

            async def handle(hass: HomeAssistant, webhook_id: str, request: web.Request) -> web.Response:
                try:
                    data = await request.json()
                except ValueError:
                    return web.Response(status=400)
                kid, what = str(data.get("id", "")), str(data.get("event", "press"))
                if kid:
                    await self.press(kid, what)
                return web.Response(status=200)

            webhook.async_register(self.hass, DOMAIN, "D200 Deck presses", self.webhook_id, handle, allowed_methods=["POST"])
            self._unsub.append(lambda: webhook.async_unregister(self.hass, self.webhook_id))
            try:
                self.webhook_url = webhook.async_generate_url(self.hass, self.webhook_id, allow_internal=True, prefer_external=False)
            except NoURLAvailableError:
                _LOGGER.error("D200 Deck: Home Assistant has no URL the deck could post presses to (Settings, System, Network)")
        await self._resync()

        async def tick(_now) -> None:
            await self._resync()

        self._unsub.append(async_track_time_interval(self.hass, tick, RESYNC))

    async def _resync(self) -> None:
        """Renew the webhook subscription; send every key again (the deck may have restarted)."""
        if self.webhook_url:
            await self._web("subscribe", self.api.subscribe(self.webhook_url, SUBSCRIPTION_TTL))
        if self.entities:
            await self.publish_all()

    async def press(self, kid: str, what: str) -> None:
        entity_id = next((e for e in self.entities if key_id(e) == kid), None)
        self.hass.bus.async_fire(EVENT_PRESS, {"id": kid, "entity_id": entity_id, "event": what, "base_topic": self.base if self.use_mqtt else self.host})
        if what != "press" or not entity_id or not self.default_actions:
            return
        st = self.hass.states.get(entity_id)
        act = default_action(entity_id, st.state if st else None)
        if act is None:
            return
        domain, service = act
        try:
            await self.hass.services.async_call(domain, service, {ATTR_ENTITY_ID: entity_id}, blocking=False)
        except HomeAssistantError as err:
            _LOGGER.warning("D200 key %s: %s.%s failed: %s", kid, domain, service, err)

    async def stop(self) -> None:
        for u in self._unsub:
            u()
        self._unsub.clear()
        if self.api and self.webhook_url:
            await self._web("unsubscribe", self.api.unsubscribe(self.webhook_url))


async def async_setup_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    bridge = Bridge(hass, entry)
    if bridge.use_mqtt and not await mqtt.async_wait_for_mqtt_client(hass):
        _LOGGER.error("MQTT is not available; set up the MQTT integration first")
        return False
    await bridge.start()
    entry.runtime_data = bridge
    entry.async_on_unload(entry.add_update_listener(_options_changed))
    _register_services(hass)
    return True


async def async_unload_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    bridge: Bridge = entry.runtime_data
    await bridge.stop()
    return True


async def _options_changed(hass: HomeAssistant, entry: ConfigEntry) -> None:
    """Entities taken off the list disappear from the deck; then start again."""
    old: Bridge = entry.runtime_data
    gone = set(old.entities) - set(entry.options.get(CONF_ENTITIES, []))
    for e in gone:
        await old.publish_key(key_id(e), None)
    await hass.config_entries.async_reload(entry.entry_id)


def _bridges(hass: HomeAssistant, deck: str | None) -> list[Bridge]:
    out = [b for b in (e.runtime_data for e in hass.config_entries.async_loaded_entries(DOMAIN)) if b.matches(deck)]
    if not out:
        raise HomeAssistantError(f"No D200 Deck {deck}" if deck else "D200 Deck is not set up")
    return out


def _register_services(hass: HomeAssistant) -> None:
    if hass.services.has_service(DOMAIN, SERVICE_NOTIFY):
        return
    deck = vol.Optional(CONF_BASE_TOPIC)

    async def notify(call: ServiceCall) -> None:
        d = call.data
        msg: dict[str, Any] = {"text": d["message"]}
        for k in ("title", "icon", "color", "duration"):
            if k in d:
                msg[k] = d[k]
        if d.get("critical"):
            msg["priority"] = "critical"
        for b in _bridges(hass, d.get(CONF_BASE_TOPIC)):
            await b.notify(msg)

    async def set_key(call: ServiceCall) -> None:
        d = dict(call.data)
        kid = d.pop("key_id")
        target = d.pop(CONF_BASE_TOPIC, None)
        for b in _bridges(hass, target):
            await b.publish_key(kid, d)

    async def clear_key(call: ServiceCall) -> None:
        for b in _bridges(hass, call.data.get(CONF_BASE_TOPIC)):
            await b.publish_key(call.data["key_id"], None)

    async def show_remote(call: ServiceCall) -> None:
        for b in _bridges(hass, call.data.get(CONF_BASE_TOPIC)):
            await b.show_remote(call.data.get("show", True))

    hass.services.async_register(DOMAIN, SERVICE_NOTIFY, notify, vol.Schema({
        vol.Required("message"): cv.string,
        vol.Optional("title"): cv.string,
        vol.Optional("icon"): cv.string,
        vol.Optional("color"): cv.string,
        vol.Optional("duration"): vol.All(vol.Coerce(float), vol.Range(min=1, max=600)),
        vol.Optional("critical", default=False): cv.boolean,
        deck: cv.string,
    }))
    hass.services.async_register(DOMAIN, SERVICE_SET_KEY, set_key, vol.Schema({vol.Required("key_id"): ID_RE, deck: cv.string, **KEY_FIELDS}))
    hass.services.async_register(DOMAIN, SERVICE_CLEAR_KEY, clear_key, vol.Schema({vol.Required("key_id"): ID_RE, deck: cv.string}))
    hass.services.async_register(DOMAIN, SERVICE_SHOW_REMOTE, show_remote, vol.Schema({vol.Optional("show", default=True): cv.boolean, deck: cv.string}))
