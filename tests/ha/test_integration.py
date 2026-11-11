"""The integration in a running Home Assistant with a mocked MQTT client."""

import json

import pytest
from pytest_homeassistant_custom_component.common import MockConfigEntry, async_capture_events, async_fire_mqtt_message, async_mock_service

from homeassistant import config_entries
from homeassistant.core import HomeAssistant
from homeassistant.data_entry_flow import FlowResultType

from custom_components.d200_deck.const import CONF_BASE_TOPIC, CONF_DEFAULT_ACTIONS, CONF_ENTITIES, DOMAIN, EVENT_PRESS


@pytest.fixture(autouse=True)
def auto_enable_custom_integrations(enable_custom_integrations):
    yield


def published(mqtt_mock, topic):
    """Payloads published to `topic`, in order: (payload, retain)."""
    return [(c.args[1], c.args[3]) for c in mqtt_mock.async_publish.call_args_list if c.args[0] == topic]


async def setup(hass, mqtt_mock, entities=("light.kitchen",), defaults=True):
    entry = MockConfigEntry(domain=DOMAIN, data={CONF_BASE_TOPIC: "d200"}, options={CONF_ENTITIES: list(entities), CONF_DEFAULT_ACTIONS: defaults}, unique_id="d200")
    entry.add_to_hass(hass)
    assert await hass.config_entries.async_setup(entry.entry_id)
    await hass.async_block_till_done()
    return entry


async def test_config_flow(hass: HomeAssistant, mqtt_mock) -> None:
    r = await hass.config_entries.flow.async_init(DOMAIN, context={"source": config_entries.SOURCE_USER})
    assert r["type"] is FlowResultType.FORM
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_BASE_TOPIC: "/bad/#", CONF_ENTITIES: []})
    assert r["errors"] == {CONF_BASE_TOPIC: "invalid_topic"}
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_BASE_TOPIC: "/desk/", CONF_ENTITIES: ["light.kitchen"]})
    assert r["type"] is FlowResultType.CREATE_ENTRY
    assert r["data"] == {CONF_BASE_TOPIC: "desk"}
    assert r["options"][CONF_ENTITIES] == ["light.kitchen"]


async def test_entities_go_to_the_deck_and_follow_their_state(hass: HomeAssistant, mqtt_mock) -> None:
    hass.states.async_set("light.kitchen", "on", {"friendly_name": "Kitchen", "rgb_color": [255, 0, 0], "brightness": 255})
    await setup(hass, mqtt_mock)
    first = published(mqtt_mock, "d200/ha/light.kitchen/set")
    assert first and first[-1][1] is True, "retained"
    assert json.loads(first[-1][0]) == {"label": "Kitchen", "kind": "light", "on": True, "color": "#FF0000", "progress": 100, "value": "100%"}

    hass.states.async_set("light.kitchen", "off", {"friendly_name": "Kitchen"})
    await hass.async_block_till_done()
    last = json.loads(published(mqtt_mock, "d200/ha/light.kitchen/set")[-1][0])
    assert last["on"] is False and last["value"] == "off"


async def test_press_runs_the_default_action_and_fires_an_event(hass: HomeAssistant, mqtt_mock) -> None:
    hass.states.async_set("light.kitchen", "on", {})
    calls = async_mock_service(hass, "light", "toggle")
    events = async_capture_events(hass, EVENT_PRESS)
    await setup(hass, mqtt_mock)
    async_fire_mqtt_message(hass, "d200/ha/light.kitchen/action", "press")
    await hass.async_block_till_done()
    assert len(calls) == 1 and calls[0].data["entity_id"] == "light.kitchen"
    assert events[-1].data == {"id": "light.kitchen", "entity_id": "light.kitchen", "event": "press", "base_topic": "d200"}

    # a long press only fires the event; so does a press on an id we did not publish
    async_fire_mqtt_message(hass, "d200/ha/light.kitchen/action", "long_press")
    async_fire_mqtt_message(hass, "d200/ha/doorbell/action", "press")
    await hass.async_block_till_done()
    assert len(calls) == 1
    assert [e.data["event"] for e in events] == ["press", "long_press", "press"]
    assert events[-1].data["entity_id"] is None


async def test_default_actions_can_be_switched_off(hass: HomeAssistant, mqtt_mock) -> None:
    hass.states.async_set("light.kitchen", "on", {})
    calls = async_mock_service(hass, "light", "toggle")
    await setup(hass, mqtt_mock, defaults=False)
    async_fire_mqtt_message(hass, "d200/ha/light.kitchen/action", "press")
    await hass.async_block_till_done()
    assert not calls


async def test_removed_entity_leaves_the_deck(hass: HomeAssistant, mqtt_mock) -> None:
    hass.states.async_set("light.kitchen", "on", {})
    hass.states.async_set("switch.fan", "off", {})
    entry = await setup(hass, mqtt_mock, entities=("light.kitchen", "switch.fan"))
    r = await hass.config_entries.options.async_init(entry.entry_id)
    r = await hass.config_entries.options.async_configure(r["flow_id"], {CONF_ENTITIES: ["switch.fan"], CONF_DEFAULT_ACTIONS: True})
    await hass.async_block_till_done()
    assert published(mqtt_mock, "d200/ha/light.kitchen/set")[-1] == ("", True)


async def test_services(hass: HomeAssistant, mqtt_mock) -> None:
    await setup(hass, mqtt_mock, entities=())
    await hass.services.async_call(DOMAIN, "notify", {"message": "Washing machine done", "title": "Laundry", "critical": True}, blocking=True)
    assert json.loads(published(mqtt_mock, "d200/notify")[-1][0]) == {"text": "Washing machine done", "title": "Laundry", "priority": "critical"}
    await hass.services.async_call(DOMAIN, "set_key", {"key_id": "doorbell", "label": "Door", "kind": "presence", "on": True}, blocking=True)
    assert published(mqtt_mock, "d200/ha/doorbell/set")[-1] == (json.dumps({"label": "Door", "kind": "presence", "on": True}), True)
    await hass.services.async_call(DOMAIN, "clear_key", {"key_id": "doorbell"}, blocking=True)
    assert published(mqtt_mock, "d200/ha/doorbell/set")[-1] == ("", True)
    await hass.services.async_call(DOMAIN, "show_remote_page", {}, blocking=True)
    assert published(mqtt_mock, "d200/remote/show")[-1][0] == "on"
    with pytest.raises(Exception):
        await hass.services.async_call(DOMAIN, "set_key", {"key_id": "bad id!"}, blocking=True)


async def test_deck_coming_online_gets_every_key(hass: HomeAssistant, mqtt_mock) -> None:
    hass.states.async_set("light.kitchen", "on", {})
    await setup(hass, mqtt_mock)
    n = len(published(mqtt_mock, "d200/ha/light.kitchen/set"))
    async_fire_mqtt_message(hass, "d200/availability", "online Studio-Mac")
    await hass.async_block_till_done()
    assert len(published(mqtt_mock, "d200/ha/light.kitchen/set")) == n + 1
