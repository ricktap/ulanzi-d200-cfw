"""Bambu Lab printers: the message for the deck, from the Bambu Lab integration's entities."""

import json

from pytest_homeassistant_custom_component.common import MockConfigEntry, async_fire_mqtt_message, async_mock_service

from homeassistant.core import HomeAssistant
from homeassistant.helpers import device_registry as dr, entity_registry as er

from custom_components.d200_deck.bambu import find_printers, model_of, payload, status
from custom_components.d200_deck.const import CONF_BASE_TOPIC, CONF_DEFAULT_ACTIONS, CONF_ENTITIES, DOMAIN



import pytest


@pytest.fixture(autouse=True)
def auto_enable_custom_integrations(enable_custom_integrations):
    yield


def published(mqtt_mock, topic):
    """Payloads published to `topic`, in order: (payload, retain)."""
    return [(c.args[1], c.args[3]) for c in mqtt_mock.async_publish.call_args_list if c.args[0] == topic]


def test_status() -> None:
    assert status("running", "printing", "on", "off") == "printing"
    assert status("running", "heatbed_preheating", "on", "off") == "heating"
    assert status("running", "auto_bed_leveling", "on", "off") == "calibrating"
    assert status("running", "paused_filament_runout", "on", "off") == "paused"
    assert status("running", "printing", "on", "on") == "error"
    assert status("pause", "paused_user", "on", "off") == "paused"
    assert status("prepare", "idle", "on", "off") == "calibrating"
    assert status("finish", "idle", "on", "off") == "complete"
    assert status("finish", "idle", "on", "on") == "complete", "an old error does not hide a finished print"
    assert status("failed", "idle", "on", "off") == "error"
    assert status("idle", "idle", "on", "off") == "idle"
    assert status("idle", "idle", "off", "off") == "offline"
    assert status("unavailable", None, None, None) == "offline"
    assert status(None, None, None, None) == "offline"


def test_models() -> None:
    assert [model_of(m) for m in ("A1MINI", "A1 Mini", "A1", "P1S", "X1C", None)] == ["a1-mini", "a1-mini", "a1", "p1s", "p1s", "p1s"]


def test_payload() -> None:
    values = {
        "status": ("running", {}), "stage": ("printing", {}), "online": ("on", {}), "error": ("off", {}),
        "progress": ("34", {}), "remaining": ("1.6", {"unit_of_measurement": "h"}),
        "nozzle": ("245", {}), "nozzle_target": ("245", {}), "bed": ("70", {}), "bed_target": ("unavailable", {}),
        "light": ("on", {}), "layer": ("102", {}), "total_layers": ("300", {}), "active_tray": ("none", {}),
    }
    trays = [
        ("Bambu PLA Matte", {"type": "PLA", "color": "#FFFFFFFF", "remain": 0, "remain_enabled": True, "active": False}),
        ("Bambu PETG", {"type": "PETG", "color": "#74b9ffFF", "remain": 65, "remain_enabled": True, "active": True}),
        ("Empty", {"empty": True}),
        ("Generic PLA", {"type": "PLA", "color": "#FF8278FF", "remain": -1, "remain_enabled": False}),
    ]
    p = payload("P1S", "P1S", values, trays, ("Bambu PLA", {"type": "PLA", "color": "#000000FF", "active": False}))
    assert p == {
        "name": "P1S", "model": "p1s", "status": "printing", "progress": 34.0, "remainingMinutes": 96,
        "nozzle": 245.0, "nozzleTarget": 245.0, "bed": 70.0, "lightOn": True, "filamentColor": "#74B9FF",
        "layer": 102.0, "totalLayers": 300.0,
        "spools": [
            {"material": "PLA", "remaining": 0.0, "color": "#FFFFFF"},
            {"material": "PETG", "remaining": 65.0, "color": "#74B9FF"},
            {"material": "PLA", "remaining": None, "color": "#FF8278"},
        ],
    }
    # no AMS: the external spool is the only spool, and the loaded filament
    ext = ("Bambu PLA Matte", {"type": "PLA", "color": "#FFFFFFFF", "remain": -1, "remain_enabled": False, "active": True})
    p = payload("A1 Mini", "A1MINI", {"status": ("finish", {}), "online": ("on", {}), "remaining": ("12", {"unit_of_measurement": "min"})}, [], ext)
    assert (p["model"], p["status"], p["remainingMinutes"], p["filamentColor"]) == ("a1-mini", "complete", 12, "#FFFFFF")
    assert p["spools"] == [{"material": "PLA", "remaining": None, "color": "#FFFFFF"}]
    assert "lightOn" not in p and "nozzle" not in p


async def bambu(hass: HomeAssistant):
    """A P1S with an AMS and the MakerWorld profile, as the Bambu Lab integration registers them (German names)."""
    entry = MockConfigEntry(domain="bambu_lab", data={})
    entry.add_to_hass(hass)
    devs, ents = dr.async_get(hass), er.async_get(hass)
    printer = devs.async_get_or_create(config_entry_id=entry.entry_id, identifiers={("bambu_lab", "01P")}, name="P1S", model="P1S", manufacturer="Bambu Lab")
    ams = devs.async_get_or_create(config_entry_id=entry.entry_id, identifiers={("bambu_lab", "AMS1")}, name="P1S AMS", model="AMS", via_device=("bambu_lab", "01P"))
    maker = devs.async_get_or_create(config_entry_id=entry.entry_id, identifiers={("bambu_lab", "mw")}, name="MakerWorld", manufacturer="MakerWorld")
    ids = {}
    for domain, key, name in [
        ("sensor", "print_status", "druckstatus"), ("sensor", "stage", "aktueller_arbeitsschritt"), ("sensor", "print_progress", "druckfortschritt"),
        ("sensor", "remaining_time", "verbleibende_zeit"), ("sensor", "nozzle_temp", "temperatur_der_duse"), ("sensor", "target_nozzle_temp", "zieltemperatur_der_duse"),
        ("sensor", "bed_temp", "druckbetttemperatur"), ("sensor", "target_bed_temp", "zieltemperatur_vom_druckbett"), ("light", "chamber_light", "druckraumbeleuchtung"),
        ("binary_sensor", "online", "online"), ("binary_sensor", "print_error", "druckfehler"),
    ]:
        ids[key] = ents.async_get_or_create(domain, "bambu_lab", f"01P_{key}", suggested_object_id=f"p1s_{name}", config_entry=entry, device_id=printer.id).entity_id
    for n in (2, 1):
        ids[f"tray_{n}"] = ents.async_get_or_create("sensor", "bambu_lab", f"P1S_01P_AMS_1_tray_{n}", suggested_object_id=f"p1s_ams_slot_{n}", config_entry=entry, device_id=ams.id).entity_id
    ents.async_get_or_create("sensor", "bambu_lab", "mw_downloads", config_entry=entry, device_id=maker.id)
    for key, value, attrs in [
        ("print_status", "running", {}), ("stage", "printing", {}), ("print_progress", "34", {}), ("remaining_time", "1.6", {"unit_of_measurement": "h"}),
        ("nozzle_temp", "245", {}), ("target_nozzle_temp", "245", {}), ("bed_temp", "70", {}), ("target_bed_temp", "70", {}),
        ("chamber_light", "on", {}), ("online", "on", {}), ("print_error", "off", {}),
        ("tray_1", "Bambu PLA", {"type": "PLA", "color": "#FFFFFFFF", "remain": 80, "remain_enabled": True, "active": True}),
        ("tray_2", "Bambu PETG", {"type": "PETG", "color": "#000000FF", "remain": 10, "remain_enabled": True}),
    ]:
        hass.states.async_set(ids[key], value, attrs)
    return printer, ids


async def test_printers_go_to_the_deck(hass: HomeAssistant, mqtt_mock) -> None:
    printer, ids = await bambu(hass)
    found = find_printers(hass)
    assert [(p.name, p.device_id) for p in found] == [("P1S", printer.id)], "the MakerWorld profile is no printer"
    assert found[0].trays == [ids["tray_1"], ids["tray_2"]]

    entry = MockConfigEntry(domain=DOMAIN, data={CONF_BASE_TOPIC: "d200"}, options={CONF_ENTITIES: [], CONF_DEFAULT_ACTIONS: True}, unique_id="d200")
    entry.add_to_hass(hass)
    assert await hass.config_entries.async_setup(entry.entry_id)
    await hass.async_block_till_done()
    topic = f"d200/bambu/{printer.id}/state"
    sent, retain = published(mqtt_mock, topic)[-1]
    msg = json.loads(sent)
    assert retain is True
    assert (msg["name"], msg["model"], msg["status"], msg["progress"], msg["remainingMinutes"], msg["lightOn"], msg["filamentColor"]) == ("P1S", "p1s", "printing", 34.0, 96, True, "#FFFFFF")
    assert [s["material"] for s in msg["spools"]] == ["PLA", "PETG"]

    # a change goes out (at most once a second per printer)
    from datetime import timedelta
    from homeassistant.util import dt as dt_util
    from pytest_homeassistant_custom_component.common import async_fire_time_changed

    n = len(published(mqtt_mock, topic))
    hass.states.async_set(ids["print_progress"], "35")
    hass.states.async_set(ids["nozzle_temp"], "244")
    await hass.async_block_till_done()
    async_fire_time_changed(hass, dt_util.utcnow() + timedelta(seconds=2))
    await hass.async_block_till_done()
    after = published(mqtt_mock, topic)
    assert len(after) == n + 1 and json.loads(after[-1][0])["progress"] == 35.0

    # a long press on the printer key: the chamber light
    calls = async_mock_service(hass, "light", "toggle")
    async_fire_mqtt_message(hass, f"d200/bambu/{printer.id}/action", "light")
    await hass.async_block_till_done()
    assert len(calls) == 1 and calls[0].data["entity_id"] == ids["chamber_light"]
    await hass.config_entries.async_unload(entry.entry_id)


async def test_no_bambu_lab_no_messages(hass: HomeAssistant, mqtt_mock) -> None:
    entry = MockConfigEntry(domain=DOMAIN, data={CONF_BASE_TOPIC: "d200"}, options={CONF_ENTITIES: [], CONF_DEFAULT_ACTIONS: True}, unique_id="d200")
    entry.add_to_hass(hass)
    assert await hass.config_entries.async_setup(entry.entry_id)
    await hass.async_block_till_done()
    assert not [c for c in mqtt_mock.async_publish.call_args_list if "/bambu/" in c.args[0]]
