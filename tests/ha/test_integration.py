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
    aioclient_mock.get(f"{DECK}/api/state", json={"page": "main"})  # unless a test registered one first
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
    assert sub[-1][0] == {"url": "http://ha.local:8123/api/webhook/hook123", "ttl": 900, "events": ["presses", "keys", "state"]}

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
    aioclient_mock.get(f"{DECK}/api/state", exc=aiohttp.ClientError())
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


STATE = {
    "usb": True, "computer": "Studio", "app": "Visual Studio Code", "profile": "Work", "profiles": ["Default", "Work"],
    "page": "Media", "pages": ["main", "Media"], "remote": False, "brightness": 180, "display": True, "screensaver": False,
    "follow": True, "notification": False, "version": "0.14.0", "latest": "0.15.0",
    "teams": {"presence": "Busy", "activity": "InAMeeting", "in_meeting": True, "muted": False},
    "media": {"playing": True, "title": "Song", "artist": "Band", "album": "LP", "player": "Spotify", "art_url": None},
    "pomodoro": {"phase": "focus", "running": True, "ends_at": "2026-11-12T12:00:00+00:00"},
}


def by_unique(hass, platform, unique_id):
    return er.async_get(hass).async_get_entity_id(platform, "mqtt", unique_id)


async def test_one_device_over_mqtt(hass: HomeAssistant, mqtt_mock) -> None:
    import pathlib

    # 0.13 made a device of its own with copies of the deck's MQTT entities; 0.14 a second
    # "Ulanzi D200" with the key event entity
    entry = MockConfigEntry(domain=DOMAIN, data={CONF_BASE_TOPIC: "d200"}, options={CONF_ENTITIES: []}, unique_id="d200")
    entry.add_to_hass(hass)
    devs, ents = dr.async_get(hass), er.async_get(hass)
    old = devs.async_get_or_create(config_entry_id=entry.entry_id, identifiers={(DOMAIN, entry.entry_id)}, name="D200 Deck")
    ents.async_get_or_create("binary_sensor", DOMAIN, f"{entry.entry_id}_connected", config_entry=entry, device_id=old.id)
    ents.async_get_or_create("sensor", DOMAIN, f"{entry.entry_id}_page", config_entry=entry, device_id=old.id)
    second = devs.async_get_or_create(config_entry_id=entry.entry_id, identifiers={("mqtt", "d200_deck")}, name="Ulanzi D200")
    ents.async_get_or_create("event", DOMAIN, f"{entry.entry_id}_key_event", config_entry=entry, device_id=second.id)
    assert await hass.config_entries.async_setup(entry.entry_id)
    await hass.async_block_till_done()

    # what the deck announces (tests/ha/discovery.json, written by the deck's own tests)
    for topic, payload in json.loads((pathlib.Path(__file__).parent / "discovery.json").read_text()).items():
        async_fire_mqtt_message(hass, topic, payload, retain=True)
    await hass.async_block_till_done()
    async_fire_mqtt_message(hass, "d200/availability", "online Studio")
    async_fire_mqtt_message(hass, "d200/profile", "Work")
    async_fire_mqtt_message(hass, "d200/brightness", "180")
    async_fire_mqtt_message(hass, "d200/state", json.dumps(STATE))
    await hass.async_block_till_done()

    # one device, the deck's; the integration has no entities and no device over MQTT
    assert devs.async_get_device(identifiers={(DOMAIN, entry.entry_id)}) is None
    assert dr.async_entries_for_config_entry(devs, entry.entry_id) == []
    assert er.async_entries_for_config_entry(ents, entry.entry_id) == []
    dev = devs.async_get_device(identifiers={("mqtt", "d200_deck")})
    assert dev and dev.name == "Ulanzi D200"

    st = lambda platform, uid: hass.states.get(by_unique(hass, platform, f"d200_deck_{uid}"))
    assert st("select", "page").state == "Media"
    assert st("select", "page").attributes["options"] == ["main", "Media"]
    assert st("select", "profile").state == "Work"
    assert float(st("number", "brightness").state) == 180
    assert (st("switch", "remote_page").state, st("switch", "display").state, st("switch", "screensaver").state) == ("off", "on", "off")
    assert st("switch", "follow").state == "on"
    assert st("binary_sensor", "usb").state == "on"
    assert st("binary_sensor", "connected").state == "on"
    assert st("sensor", "host").state == "Studio"
    assert st("sensor", "app").state == "Visual Studio Code"
    assert st("sensor", "teams_presence").state == "Busy"
    assert st("binary_sensor", "teams_meeting").state == "on"
    assert st("sensor", "now_playing").state == "Song – Band"
    assert st("sensor", "now_playing").attributes["album"] == "LP"
    assert st("sensor", "pomodoro_phase").state == "focus"
    up = st("update", "app")
    assert (up.state, up.attributes["installed_version"], up.attributes["latest_version"]) == ("on", "0.14.0", "0.15.0")
    assert not any(t.startswith("homeassistant/device_automation") and p for t, p in json.loads((pathlib.Path(__file__).parent / "discovery.json").read_text()).items())

    # controls go to the deck's topics
    await hass.services.async_call("select", "select_option", {"entity_id": st("select", "page").entity_id, "option": "main"}, blocking=True)
    assert published(mqtt_mock, "d200/page/set")[-1][0] == "main"
    await hass.services.async_call("switch", "turn_off", {"entity_id": st("switch", "display").entity_id}, blocking=True)
    assert published(mqtt_mock, "d200/display/set")[-1][0] == "off"
    await hass.services.async_call("button", "press", {"entity_id": st("button", "pomodoro_reset").entity_id}, blocking=True)
    assert published(mqtt_mock, "d200/pomodoro/set")[-1][0] == "reset"

    # one event entity per key on the deck's device
    k3, wide = st("event", "key_3"), st("event", "key_13")
    assert k3.name == "Ulanzi D200 Key 3" and wide.name == "Ulanzi D200 Wide key"
    assert ents.async_get(k3.entity_id).device_id == dev.id
    async_fire_mqtt_message(hass, "d200/key/3", json.dumps({"event_type": "double_press", "profile": "Work", "page": "Main"}))
    await hass.async_block_till_done()
    s3 = st("event", "key_3")
    assert (s3.attributes["event_type"], s3.attributes["profile"], s3.attributes["page"]) == ("double_press", "Work", "Main")
    assert st("event", "key_4").state == "unknown"
    # one entity for every key, with everything as attributes
    anykey = st("event", "key_pressed")
    assert anykey.name == "Ulanzi D200 Key pressed" and anykey.attributes["event_types"] == ["press", "long_press", "double_press"]
    async_fire_mqtt_message(hass, "d200/key_event", json.dumps({"event_type": "long_press", "key": 5, "profile": "Work", "page": "Main",
                                                                 "key_label": "Lamp", "key_icon": "core:light", "app": None}))
    await hass.async_block_till_done()
    a = st("event", "key_pressed").attributes
    assert (a["event_type"], a["key"], a["page"], a["profile"], a["key_label"], a["key_icon"]) == ("long_press", 5, "Main", "Work", "Lamp", "core:light")
    assert "icon" not in a or a["icon"] == "mdi:gesture-tap-button"
    await hass.services.async_call(DOMAIN, "press_key", {"key": 4, "event": "long_press"}, blocking=True)
    assert json.loads(published(mqtt_mock, "d200/press")[-1][0]) == {"key": 4, "event": "long_press"}
    await hass.services.async_call(DOMAIN, "run_action", {"action": "core:page", "params": {"page": "Media"}}, blocking=True)
    assert json.loads(published(mqtt_mock, "d200/action")[-1][0]) == {"action": "core:page", "params": {"page": "Media"}}
    with pytest.raises(Exception):
        await hass.services.async_call(DOMAIN, "run_action", {"action": "com.deckling.core:command", "params": {"cmd": "rm"}}, blocking=True)


async def test_web_api_only_device(hass: HomeAssistant, aioclient_mock, hass_client_no_auth) -> None:
    aioclient_mock.get(f"{DECK}/api/state", json=STATE)
    for a in ("core:display", "core:page", "core:back", "core:brightness", "com.deckling.core:pomodoro"):
        aioclient_mock.post(f"{DECK}/api/actions/{a}", json={"ran": a})
    aioclient_mock.post(f"{DECK}/api/keys/4/press", json={"pressed": 4})
    entry = await setup_web(hass, aioclient_mock, entities=())
    assert web_calls(aioclient_mock, "POST", "/api/subscriptions")[-1][0]["events"] == ["presses", "keys", "state"]
    dev = dr.async_get(hass).async_get_device(identifiers={(DOMAIN, entry.entry_id)})
    assert dev and dev.name == "Ulanzi D200"
    ents = {e.unique_id.removeprefix(entry.entry_id + "_"): e.entity_id for e in er.async_entries_for_config_entry(er.async_get(hass), entry.entry_id)}
    assert "key_event" not in ents and "key_13" in ents
    st = lambda k: hass.states.get(ents[k])
    assert st("connected").state == "on" and st("usb").state == "on"
    assert st("page").state == "Media" and st("page").attributes["options"] == ["main", "Media"]
    assert st("profile").state == "Work" and st("brightness").state == "180"
    assert st("display").state == "on" and st("remote_page").state == "off" and st("follow").state == "on"
    assert st("app").state == "Visual Studio Code" and st("computer").state == "Studio"
    assert st("teams_presence").state == "Busy" and st("teams_meeting").state == "on"
    assert st("now_playing").state == "Song – Band" and st("pomodoro").state == "focus"
    assert st("app_update").state == "on"

    # a state change pushed to the webhook
    client = await hass_client_no_auth()
    pushed = {**STATE, "type": "state", "display": False, "remote": True, "teams": None, "media": None}
    assert (await client.post("/api/webhook/hook123", json=pushed)).status == 200
    await hass.async_block_till_done()
    assert st("display").state == "off" and st("remote_page").state == "on"
    assert st("teams_meeting").state == "unknown" and st("now_playing").state == "idle"

    # controls become deck actions
    await hass.services.async_call("switch", "turn_on", {"entity_id": ents["display"]}, blocking=True)
    assert web_calls(aioclient_mock, "POST", "/api/actions/core:display")[-1][0] == {"params": {"on": "on"}}
    await hass.services.async_call("switch", "turn_off", {"entity_id": ents["remote_page"]}, blocking=True)
    assert web_calls(aioclient_mock, "POST", "/api/actions/core:back")
    await hass.services.async_call("select", "select_option", {"entity_id": ents["page"], "option": "main"}, blocking=True)
    assert web_calls(aioclient_mock, "POST", "/api/actions/core:page")[-1][0] == {"params": {"page": "main"}}
    await hass.services.async_call("number", "set_value", {"entity_id": ents["brightness"], "value": 90}, blocking=True)
    assert web_calls(aioclient_mock, "POST", "/api/actions/core:brightness")[-1][0] == {"params": {"value": "90"}}
    await hass.services.async_call("button", "press", {"entity_id": ents["pomodoro_toggle"]}, blocking=True)
    assert web_calls(aioclient_mock, "POST", "/api/actions/com.deckling.core:pomodoro")[-1][0] == {"params": {"do": "press"}}
    await hass.services.async_call(DOMAIN, "press_key", {"key": 4}, blocking=True)
    assert web_calls(aioclient_mock, "POST", "/api/keys/4/press")[-1][0] == {"event": "press"}

    # key events over the webhook
    body = {"type": "key", "key": 7, "event": "long_press", "profile": "Default", "page": "remote", "n": 4, "label": "Door", "icon": "lock"}
    assert (await client.post("/api/webhook/hook123", json=body)).status == 200
    await hass.async_block_till_done()
    ev = st("key_7")
    assert ev.attributes["event_type"] == "long_press" and ev.attributes["page"] == "remote"
    a = st("key_pressed").attributes
    assert (a["event_type"], a["key"], a["key_label"], a["key_icon"]) == ("long_press", 7, "Door", "lock")
    assert st("key_6").state == "unknown" and hass.states.get(ents["key_13"]).name == "Ulanzi D200 Wide key"
