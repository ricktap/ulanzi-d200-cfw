"""Set up D200 Deck: the MQTT base topic, then which entities go on the deck."""

from __future__ import annotations

from typing import Any

import voluptuous as vol

from homeassistant.config_entries import ConfigEntry, ConfigFlow, ConfigFlowResult, OptionsFlow
from homeassistant.core import callback
from homeassistant.helpers import selector

from .const import CONF_BASE_TOPIC, CONF_DEFAULT_ACTIONS, CONF_ENTITIES, DEFAULT_BASE_TOPIC, DOMAIN


def _clean(topic: str) -> str:
    return topic.strip().strip("/")


class D200DeckConfigFlow(ConfigFlow, domain=DOMAIN):
    VERSION = 1

    async def async_step_user(self, user_input: dict[str, Any] | None = None) -> ConfigFlowResult:
        errors: dict[str, str] = {}
        if user_input is not None:
            base = _clean(user_input[CONF_BASE_TOPIC])
            if not base or any(c in base for c in "+#"):
                errors[CONF_BASE_TOPIC] = "invalid_topic"
            else:
                await self.async_set_unique_id(base)
                self._abort_if_unique_id_configured()
                return self.async_create_entry(
                    title=f"D200 Deck ({base})",
                    data={CONF_BASE_TOPIC: base},
                    options={CONF_ENTITIES: user_input.get(CONF_ENTITIES, []), CONF_DEFAULT_ACTIONS: True},
                )
        return self.async_show_form(
            step_id="user",
            data_schema=vol.Schema({
                vol.Required(CONF_BASE_TOPIC, default=DEFAULT_BASE_TOPIC): str,
                vol.Optional(CONF_ENTITIES, default=[]): selector.EntitySelector(selector.EntitySelectorConfig(multiple=True)),
            }),
            errors=errors,
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
