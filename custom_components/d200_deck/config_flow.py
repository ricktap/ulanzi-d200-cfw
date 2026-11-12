"""Set up D200 Deck: MQTT, the deck's Web API or both, then the entities for the deck."""

from __future__ import annotations

from typing import Any

import voluptuous as vol

from homeassistant.components import webhook
from homeassistant.config_entries import ConfigEntry, ConfigFlow, ConfigFlowResult, OptionsFlow
from homeassistant.core import callback
from homeassistant.helpers import selector
from homeassistant.helpers.service_info.zeroconf import ZeroconfServiceInfo

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
    TRANSPORT_BOTH,
    TRANSPORT_MQTT,
    TRANSPORT_WEB,
)
from .mqtt_share import broker_for_deck, ha_broker
from .web import DeckApi, WebError

CONF_SETUP_MQTT = "setup_mqtt"


def _clean(topic: str) -> str:
    return topic.strip().strip("/")


class D200DeckConfigFlow(ConfigFlow, domain=DOMAIN):
    VERSION = 1

    def __init__(self) -> None:
        self._data: dict[str, Any] = {}

    async def async_step_user(self, user_input: dict[str, Any] | None = None) -> ConfigFlowResult:
        if user_input is not None:
            self._data[CONF_TRANSPORT] = user_input[CONF_TRANSPORT]
            if user_input[CONF_TRANSPORT] in (TRANSPORT_MQTT, TRANSPORT_BOTH):
                return await self.async_step_mqtt()
            return await self.async_step_web()
        return self.async_show_form(
            step_id="user",
            data_schema=vol.Schema({
                vol.Required(CONF_TRANSPORT, default=TRANSPORT_MQTT): selector.SelectSelector(selector.SelectSelectorConfig(
                    options=[TRANSPORT_MQTT, TRANSPORT_WEB, TRANSPORT_BOTH], translation_key=CONF_TRANSPORT, mode=selector.SelectSelectorMode.LIST)),
            }),
        )

    async def async_step_mqtt(self, user_input: dict[str, Any] | None = None) -> ConfigFlowResult:
        errors: dict[str, str] = {}
        if user_input is not None:
            base = _clean(user_input[CONF_BASE_TOPIC])
            if not base or any(c in base for c in "+#"):
                errors[CONF_BASE_TOPIC] = "invalid_topic"
            else:
                self._data[CONF_BASE_TOPIC] = base
                if self._data[CONF_TRANSPORT] == TRANSPORT_BOTH:
                    return await self.async_step_web()
                await self.async_set_unique_id(base)
                self._abort_if_unique_id_configured()
                return await self.async_step_entities()
        return self.async_show_form(
            step_id="mqtt",
            data_schema=vol.Schema({vol.Required(CONF_BASE_TOPIC, default=DEFAULT_BASE_TOPIC): str}),
            errors=errors,
        )

    async def _connect_web(self, host: str, port: int, token: str, setup_mqtt: bool) -> str | None:
        """Check the token; hand Home Assistant's broker to the deck if asked. Returns an error key."""
        api = DeckApi(self.hass, host, port, token)
        try:
            await api.info()
            if setup_mqtt:
                broker = await broker_for_deck(self.hass)
                if broker is None:
                    return "no_mqtt"
                await api.set_mqtt(broker)
                deck = await api.mqtt()
                self._data[CONF_BASE_TOPIC] = (deck.get("base_topic") or DEFAULT_BASE_TOPIC).strip("/")
                self._data[CONF_TRANSPORT] = TRANSPORT_BOTH
        except WebError as err:
            return "invalid_auth" if err.auth else "cannot_connect"
        self._data.update({CONF_HOST: host, CONF_PORT: port, CONF_TOKEN: token, CONF_WEBHOOK_ID: webhook.async_generate_id()})
        return None

    def _web_schema(self, with_host: bool) -> vol.Schema:
        fields: dict[Any, Any] = {}
        if with_host:
            fields[vol.Required(CONF_HOST)] = str
            fields[vol.Required(CONF_PORT, default=DEFAULT_PORT)] = vol.All(vol.Coerce(int), vol.Range(min=1, max=65535))
        fields[vol.Required(CONF_TOKEN)] = selector.TextSelector(selector.TextSelectorConfig(type=selector.TextSelectorType.PASSWORD))
        # only offered when the deck is reached over the Web API alone and Home Assistant has MQTT
        if self._data.get(CONF_TRANSPORT) != TRANSPORT_BOTH and ha_broker(self.hass) is not None:
            fields[vol.Required(CONF_SETUP_MQTT, default=True)] = bool
        return vol.Schema(fields)

    async def async_step_web(self, user_input: dict[str, Any] | None = None) -> ConfigFlowResult:
        errors: dict[str, str] = {}
        if user_input is not None:
            host = user_input[CONF_HOST].strip().removeprefix("http://").removeprefix("https://").strip("/")
            port, token = user_input[CONF_PORT], user_input[CONF_TOKEN].strip()
            err = await self._connect_web(host, port, token, user_input.get(CONF_SETUP_MQTT, False))
            if err:
                errors["base"] = err
            else:
                await self.async_set_unique_id(self._data.get(CONF_BASE_TOPIC) or f"{host}:{port}")
                self._abort_if_unique_id_configured()
                return await self.async_step_entities()
        return self.async_show_form(step_id="web", data_schema=self._web_schema(True), errors=errors)

    # -------------------------------------------------------------- discovered on the network

    async def async_step_zeroconf(self, discovery_info: ZeroconfServiceInfo) -> ConfigFlowResult:
        """D200 Deck announces itself (_d200deck._tcp) while its Web API runs."""
        deck_id = discovery_info.properties.get("id") or discovery_info.hostname
        host, port = discovery_info.host, discovery_info.port or DEFAULT_PORT
        await self.async_set_unique_id(f"deck-{deck_id}")
        self._abort_if_unique_id_configured(updates={CONF_HOST: host, CONF_PORT: port})
        self._data = {CONF_TRANSPORT: TRANSPORT_WEB, CONF_HOST: host, CONF_PORT: port}
        self.context["title_placeholders"] = {"name": discovery_info.name.split("._d200deck")[0]}
        return await self.async_step_zeroconf_confirm()

    async def async_step_zeroconf_confirm(self, user_input: dict[str, Any] | None = None) -> ConfigFlowResult:
        errors: dict[str, str] = {}
        host, port = self._data[CONF_HOST], self._data[CONF_PORT]
        if user_input is not None:
            err = await self._connect_web(host, port, user_input[CONF_TOKEN].strip(), user_input.get(CONF_SETUP_MQTT, False))
            if err:
                errors["base"] = err
            else:
                return await self.async_step_entities()
        return self.async_show_form(
            step_id="zeroconf_confirm",
            data_schema=self._web_schema(False),
            errors=errors,
            description_placeholders={"host": f"{host}:{port}"},
        )

    async def async_step_entities(self, user_input: dict[str, Any] | None = None) -> ConfigFlowResult:
        if user_input is not None:
            target = self._data.get(CONF_BASE_TOPIC) or self._data.get(CONF_HOST)
            return self.async_create_entry(
                title=f"D200 Deck ({target})",
                data=self._data,
                options={CONF_ENTITIES: user_input.get(CONF_ENTITIES, []), CONF_DEFAULT_ACTIONS: True},
            )
        return self.async_show_form(
            step_id="entities",
            data_schema=vol.Schema({
                vol.Optional(CONF_ENTITIES, default=[]): selector.EntitySelector(selector.EntitySelectorConfig(multiple=True)),
            }),
        )

    @staticmethod
    @callback
    def async_get_options_flow(config_entry: ConfigEntry) -> OptionsFlow:
        return D200DeckOptionsFlow()


class D200DeckOptionsFlow(OptionsFlow):
    async def async_step_init(self, user_input: dict[str, Any] | None = None) -> ConfigFlowResult:
        if user_input is not None:
            return self.async_create_entry(data=user_input)
        opts = self.config_entry.options
        return self.async_show_form(
            step_id="init",
            data_schema=vol.Schema({
                vol.Optional(CONF_ENTITIES, default=opts.get(CONF_ENTITIES, [])): selector.EntitySelector(selector.EntitySelectorConfig(multiple=True)),
                vol.Optional(CONF_DEFAULT_ACTIONS, default=opts.get(CONF_DEFAULT_ACTIONS, True)): bool,
            }),
        )
