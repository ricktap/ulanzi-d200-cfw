"""The entity → key translation, without Home Assistant."""

from custom_components.d200_deck.payload import default_action, entity_payload, key_id


def test_light_on_with_colour_and_brightness():
    p = entity_payload("light.kitchen", "on", {"friendly_name": "Kitchen", "rgb_color": [255, 200, 100], "brightness": 128})
    assert p == {"label": "Kitchen", "kind": "light", "on": True, "color": "#FFC864", "progress": 50, "value": "50%"}


def test_light_off():
    p = entity_payload("light.kitchen", "off", {"friendly_name": "Kitchen"})
    assert p["on"] is False and p["value"] == "off" and "color" not in p


def test_sensors():
    assert entity_payload("sensor.t", "21.456", {"unit_of_measurement": "°C"})["value"] == "21.5°C"
    hum = entity_payload("sensor.h", "48", {"unit_of_measurement": "%"})
    assert hum["value"] == "48%" and hum["progress"] == 48
    assert entity_payload("sensor.p", "1200", {"unit_of_measurement": "W", "device_class": "power"}) == {"label": "sensor.p", "kind": "energy", "value": "1200 W"}
    assert entity_payload("sensor.s", "washing", {})["value"] == "washing"


def test_lock_cover_climate_person():
    assert entity_payload("lock.door", "unlocked", {})["kind"] == "unlock"
    assert entity_payload("lock.door", "locked", {})["kind"] == "lock"
    cov = entity_payload("cover.blind", "open", {"current_position": 70})
    assert cov["on"] is True and cov["progress"] == 70 and cov["value"] == "70%"
    cl = entity_payload("climate.living", "heat", {"friendly_name": "Living", "current_temperature": 20.5, "temperature": 22})
    assert cl["value"] == "20.5°" and cl["label"] == "Living → 22°" and cl["on"] is True
    assert entity_payload("person.anna", "not_home", {})["value"] == "away"


def test_unavailable_and_gone():
    assert entity_payload("light.x", "unavailable", {})["value"] == "n/a"
    assert entity_payload("light.x", None, None) == {"label": "light.x", "kind": "light", "value": "n/a"}


def test_default_actions():
    assert default_action("light.a", "on") == ("light", "toggle")
    assert default_action("scene.movie", "scening") == ("scene", "turn_on")
    assert default_action("button.ring", None) == ("button", "press")
    assert default_action("lock.door", "locked") == ("lock", "unlock")
    assert default_action("lock.door", "unlocked") == ("lock", "lock")
    assert default_action("vacuum.v", "cleaning") == ("vacuum", "return_to_base")
    assert default_action("sensor.t", "1") is None


def test_key_id():
    assert key_id("light.kitchen") == "light.kitchen"
    assert key_id("sensor.a b") == "sensor.a_b"
