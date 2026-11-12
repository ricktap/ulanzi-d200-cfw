"""The integration in a running Home Assistant with a mocked MQTT client."""

import json

import pytest
from pytest_homeassistant_custom_component.common import MockConfigEntry, async_capture_events, async_fire_mqtt_message, async_mock_service

from homeassistant import config_entries
from homeassistant.core import HomeAssistant
from homeassistant.data_entry_flow import FlowResultType

import aiohttp
from homeassistant.core_config import async_process_ha_core_config
from homeassistant.setup import async_setup_component

from custom_components.d200_deck.const import (
    CONF_BASE_TOPIC, CONF_DEFAULT_ACTIONS, CONF_ENTITIES, CONF_HOST, CONF_PORT, CONF_TOKEN, CONF_TRANSPORT, CONF_WEBHOOK_ID, DOMAIN, EVENT_PRESS,
)

DECK = "http://deck.local:8765"


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


async def test_config_flow_mqtt(hass: HomeAssistant, mqtt_mock) -> None:
    r = await hass.config_entries.flow.async_init(DOMAIN, context={"source": config_entries.SOURCE_USER})
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_TRANSPORT: "mqtt"})
    assert r["step_id"] == "mqtt"
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_BASE_TOPIC: "/bad/#"})
    assert r["errors"] == {CONF_BASE_TOPIC: "invalid_topic"}
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_BASE_TOPIC: "/desk/"})
    assert r["step_id"] == "entities"
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_ENTITIES: ["light.kitchen"]})
    assert r["type"] is FlowResultType.CREATE_ENTRY
    assert r["data"] == {CONF_TRANSPORT: "mqtt", CONF_BASE_TOPIC: "desk"}
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


# ------------------------------------------------------------------ Web API


async def test_config_flow_web(hass: HomeAssistant, aioclient_mock) -> None:
    r = await hass.config_entries.flow.async_init(DOMAIN, context={"source": config_entries.SOURCE_USER})
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_TRANSPORT: "web"})
    assert r["step_id"] == "web"
    aioclient_mock.get(f"{DECK}/api", exc=aiohttp.ClientError())
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_HOST: "deck.local", CONF_PORT: 8765, CONF_TOKEN: "t"})
    assert r["errors"] == {"base": "cannot_connect"}
    aioclient_mock.clear_requests()
    aioclient_mock.get(f"{DECK}/api", status=401, json={"error": "missing or wrong token"})
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_HOST: "deck.local", CONF_PORT: 8765, CONF_TOKEN: "t"})
    assert r["errors"] == {"base": "invalid_auth"}
    aioclient_mock.clear_requests()
    aioclient_mock.get(f"{DECK}/api", json={"name": "D200 Deck"})
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_HOST: "http://deck.local/", CONF_PORT: 8765, CONF_TOKEN: " t "})
    assert r["step_id"] == "entities"
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_ENTITIES: []})
    assert r["type"] is FlowResultType.CREATE_ENTRY
    d = r["data"]
    assert (d[CONF_TRANSPORT], d[CONF_HOST], d[CONF_PORT], d[CONF_TOKEN]) == ("web", "deck.local", 8765, "t")
    assert d[CONF_WEBHOOK_ID]
    assert aioclient_mock.mock_calls[-1][3]["Authorization"] == "Bearer t"


def web_calls(aioclient_mock, method, path):
    return [(c[2], c[3]) for c in aioclient_mock.mock_calls if c[0] == method and str(c[1]) == DECK + path]


async def setup_web(hass, aioclient_mock, entities=("light.kitchen",)):
    await async_process_ha_core_config(hass, {"internal_url": "http://ha.local:8123"})
    assert await async_setup_component(hass, "webhook", {})
    for e in entities:
        aioclient_mock.put(f"{DECK}/api/keys/{e}", json={"key": e})
        aioclient_mock.delete(f"{DECK}/api/keys/{e}", json={"removed": e})
    aioclient_mock.get(f"{DECK}/api", json={"name": "D200 Deck"})
    aioclient_mock.post(f"{DECK}/api/subscriptions", json={"expires_in": 900})
    aioclient_mock.delete(f"{DECK}/api/subscriptions", json={"removed": True})
    aioclient_mock.post(f"{DECK}/api/notify", json={"shown": True})
    aioclient_mock.post(f"{DECK}/api/actions/com.deckling.ha:remote", json={"ran": True})
    data = {CONF_TRANSPORT: "web", CONF_HOST: "deck.local", CONF_PORT: 8765, CONF_TOKEN: "t", CONF_WEBHOOK_ID: "hook123"}
    entry = MockConfigEntry(domain=DOMAIN, data=data, options={CONF_ENTITIES: list(entities), CONF_DEFAULT_ACTIONS: True}, unique_id="deck.local:8765")
    entry.add_to_hass(hass)
    assert await hass.config_entries.async_setup(entry.entry_id)
    await hass.async_block_till_done()
    return entry


async def test_web_keys_subscription_and_presses(hass: HomeAssistant, aioclient_mock, hass_client_no_auth) -> None:
    hass.states.async_set("light.kitchen", "on", {"friendly_name": "Kitchen"})
    calls = async_mock_service(hass, "light", "toggle")
    events = async_capture_events(hass, EVENT_PRESS)
    entry = await setup_web(hass, aioclient_mock)

    put = web_calls(aioclient_mock, "PUT", "/api/keys/light.kitchen")
    assert put and put[-1][0] == {"label": "Kitchen", "kind": "light", "on": True, "value": "on"}
    assert put[-1][1]["Authorization"] == "Bearer t"
    sub = web_calls(aioclient_mock, "POST", "/api/subscriptions")
    assert sub[-1][0] == {"url": "http://ha.local:8123/api/webhook/hook123", "ttl": 900, "events": ["presses", "keys"]}

    # the deck posts a press to the webhook
    client = await hass_client_no_auth()
    resp = await client.post("/api/webhook/hook123", json={"id": "light.kitchen", "event": "press"})
    assert resp.status == 200
    await hass.async_block_till_done()
    assert len(calls) == 1
    assert events[-1].data == {"id": "light.kitchen", "entity_id": "light.kitchen", "event": "press", "base_topic": "deck.local"}

    # state changes go out as PUTs
    hass.states.async_set("light.kitchen", "off", {"friendly_name": "Kitchen"})
    await hass.async_block_till_done()
    assert web_calls(aioclient_mock, "PUT", "/api/keys/light.kitchen")[-1][0]["on"] is False

    # services over the Web API
    await hass.services.async_call(DOMAIN, "notify", {"message": "Done"}, blocking=True)
    assert web_calls(aioclient_mock, "POST", "/api/notify")[-1][0] == {"text": "Done"}
    await hass.services.async_call(DOMAIN, "show_remote_page", {}, blocking=True)
    assert web_calls(aioclient_mock, "POST", "/api/actions/com.deckling.ha:remote")

    # unloading drops the subscription
    await hass.config_entries.async_unload(entry.entry_id)
    await hass.async_block_till_done()
    assert web_calls(aioclient_mock, "DELETE", "/api/subscriptions")[-1][0] == {"url": "http://ha.local:8123/api/webhook/hook123"}


async def test_web_resync_renews_and_resends(hass: HomeAssistant, aioclient_mock) -> None:
    from datetime import timedelta
    from homeassistant.util import dt as dt_util
    from pytest_homeassistant_custom_component.common import async_fire_time_changed

    hass.states.async_set("light.kitchen", "on", {})
    await setup_web(hass, aioclient_mock)
    n_sub = len(web_calls(aioclient_mock, "POST", "/api/subscriptions"))
    n_put = len(web_calls(aioclient_mock, "PUT", "/api/keys/light.kitchen"))
    async_fire_time_changed(hass, dt_util.utcnow() + timedelta(minutes=6))
    await hass.async_block_till_done()
    assert len(web_calls(aioclient_mock, "POST", "/api/subscriptions")) == n_sub + 1
    assert len(web_calls(aioclient_mock, "PUT", "/api/keys/light.kitchen")) == n_put + 1


async def test_deck_offline_does_not_break_setup(hass: HomeAssistant, aioclient_mock) -> None:
    await async_process_ha_core_config(hass, {"internal_url": "http://ha.local:8123"})
    assert await async_setup_component(hass, "webhook", {})
    aioclient_mock.get(f"{DECK}/api", exc=aiohttp.ClientError())
    aioclient_mock.put(f"{DECK}/api/keys/light.kitchen", exc=aiohttp.ClientError())
    aioclient_mock.post(f"{DECK}/api/subscriptions", exc=aiohttp.ClientError())
    hass.states.async_set("light.kitchen", "on", {})
    data = {CONF_TRANSPORT: "web", CONF_HOST: "deck.local", CONF_PORT: 8765, CONF_TOKEN: "t", CONF_WEBHOOK_ID: "h"}
    entry = MockConfigEntry(domain=DOMAIN, data=data, options={CONF_ENTITIES: ["light.kitchen"]})
    entry.add_to_hass(hass)
    assert await hass.config_entries.async_setup(entry.entry_id)


# ------------------------------------------------------------------ discovery and MQTT hand-over

from unittest.mock import patch

from homeassistant.helpers.service_info.zeroconf import ZeroconfServiceInfo
from ipaddress import ip_address

DISCOVERED = ZeroconfServiceInfo(
    ip_address=ip_address("192.168.1.20"), ip_addresses=[ip_address("192.168.1.20")], port=8765,
    hostname="studio-d200deck.local.", type="_d200deck._tcp.local.", name="D200 Deck on studio._d200deck._tcp.local.",
    properties={"id": "studio", "version": "0.12.0", "path": "/api"},
)
FOUND = "http://192.168.1.20:8765"


async def test_discovered_deck_gets_home_assistants_broker(hass: HomeAssistant, aioclient_mock) -> None:
    # Home Assistant's MQTT runs on the Mosquitto add-on: the deck gets HA's address instead
    MockConfigEntry(domain="mqtt", data={"broker": "core-mosquitto", "port": 1883, "username": "addons", "password": "secret"}).add_to_hass(hass)
    aioclient_mock.get(f"{FOUND}/api", json={"name": "D200 Deck"})
    aioclient_mock.put(f"{FOUND}/api/mqtt", json={"saved": True})
    aioclient_mock.get(f"{FOUND}/api/mqtt", json={"enabled": True, "base_topic": "desk"})
    r = await hass.config_entries.flow.async_init(DOMAIN, context={"source": config_entries.SOURCE_ZEROCONF}, data=DISCOVERED)
    assert r["step_id"] == "zeroconf_confirm"
    with patch("custom_components.d200_deck.mqtt_share.async_get_source_ip", return_value="192.168.1.5"):
        r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_TOKEN: "t", "setup_mqtt": True})
    assert r["step_id"] == "entities"
    put = [c for c in aioclient_mock.mock_calls if c[0] == "PUT"][-1]
    assert put[2] == {"host": "192.168.1.5", "port": 1883, "username": "addons", "password": "secret", "tls": False}
    # (the flow is under test here, not the setup, which would wait for an MQTT client)
    with patch("custom_components.d200_deck.async_setup_entry", return_value=True):
        r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_ENTITIES: []})
    assert r["type"] is FlowResultType.CREATE_ENTRY
    d = r["data"]
    assert (d[CONF_TRANSPORT], d[CONF_BASE_TOPIC], d[CONF_HOST], d[CONF_PORT]) == ("both", "desk", "192.168.1.20", 8765)
    assert r["result"].unique_id == "deck-studio"

    # the same deck announcing itself again (new address) updates the entry instead
    moved = ZeroconfServiceInfo(
        ip_address=ip_address("192.168.1.21"), ip_addresses=[ip_address("192.168.1.21")], port=8765,
        hostname=DISCOVERED.hostname, type=DISCOVERED.type, name=DISCOVERED.name, properties=DISCOVERED.properties,
    )
    r = await hass.config_entries.flow.async_init(DOMAIN, context={"source": config_entries.SOURCE_ZEROCONF}, data=moved)
    assert r["type"] is FlowResultType.ABORT
    assert hass.config_entries.async_entries(DOMAIN)[0].data[CONF_HOST] == "192.168.1.21"


async def test_discovered_deck_without_mqtt_hand_over(hass: HomeAssistant, aioclient_mock) -> None:
    aioclient_mock.get(f"{FOUND}/api", status=401, json={"error": "wrong token"})
    r = await hass.config_entries.flow.async_init(DOMAIN, context={"source": config_entries.SOURCE_ZEROCONF}, data=DISCOVERED)
    assert "setup_mqtt" not in r["data_schema"].schema, "no MQTT in Home Assistant: nothing to hand over"
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_TOKEN: "bad"})
    assert r["errors"] == {"base": "invalid_auth"}
    aioclient_mock.clear_requests()
    aioclient_mock.get(f"{FOUND}/api", json={"name": "D200 Deck"})
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_TOKEN: "t"})
    r = await hass.config_entries.flow.async_configure(r["flow_id"], {CONF_ENTITIES: []})
    assert r["data"][CONF_TRANSPORT] == "web"
    assert not [c for c in aioclient_mock.mock_calls if c[0] == "PUT"]


# ------------------------------------------------------------------ the device and its entities

from homeassistant.helpers import device_registry as dr, entity_registry as er


def entity(hass, entry, key):
    return er.async_get(hass).async_get_entity_id(
        {"key_event": "event", "connected": "binary_sensor"}.get(key, "sensor"), DOMAIN, f"{entry.entry_id}_{key}"
    )


async def test_device_and_entities_over_mqtt(hass: HomeAssistant, mqtt_mock) -> None:
    entry = await setup(hass, mqtt_mock, entities=())
    dev = dr.async_get(hass).async_get_device(identifiers={(DOMAIN, entry.entry_id)})
    assert dev and dev.name == "D200 Deck" and dev.manufacturer == "Ulanzi"
    ev, conn, prof, page, comp = (entity(hass, entry, k) for k in ("key_event", "connected", "profile", "page", "computer"))
    assert all((ev, conn, prof, page, comp))
    assert hass.states.get(ev).attributes["event_types"] == ["press", "long_press", "double_press"]

    async_fire_mqtt_message(hass, "d200/availability", "online Studio-Mac")
    async_fire_mqtt_message(hass, "d200/page", json.dumps({"profile": "Work", "page": "Media", "remote": False}))
    await hass.async_block_till_done()
    assert hass.states.get(conn).state == "on"
    assert hass.states.get(comp).state == "Studio-Mac"
    assert (hass.states.get(prof).state, hass.states.get(page).state) == ("Work", "Media")

    # any key, not just Home Assistant ones; down and up are left out
    async_fire_mqtt_message(hass, "d200/key", json.dumps({"profile": "Work", "page": "Main", "key": 3, "event": "down"}))
    await hass.async_block_till_done()
    assert hass.states.get(ev).state == "unknown"
    async_fire_mqtt_message(hass, "d200/key", json.dumps({"profile": "Work", "page": "Main", "key": 3, "event": "double_press"}))
    await hass.async_block_till_done()
    st = hass.states.get(ev)
    assert st.attributes["event_type"] == "double_press"
    assert (st.attributes["key"], st.attributes["profile"], st.attributes["page"]) == (3, "Work", "Main")
    assert hass.states.get(page).state == "Main"

    async_fire_mqtt_message(hass, "d200/availability", "offline")
    await hass.async_block_till_done()
    assert hass.states.get(conn).state == "off"
    assert hass.states.get(comp).state == "unknown"


async def test_key_events_over_the_web_api(hass: HomeAssistant, aioclient_mock, hass_client_no_auth) -> None:
    entry = await setup_web(hass, aioclient_mock, entities=())
    assert hass.states.get(entity(hass, entry, "connected")).state == "on"
    client = await hass_client_no_auth()
    body = {"type": "key", "key": 7, "event": "long_press", "profile": "Default", "page": "remote", "n": 4}
    assert (await client.post("/api/webhook/hook123", json=body)).status == 200
    await hass.async_block_till_done()
    st = hass.states.get(entity(hass, entry, "key_event"))
    assert st.attributes["event_type"] == "long_press" and st.attributes["key"] == 7
    assert hass.states.get(entity(hass, entry, "page")).state == "remote"
