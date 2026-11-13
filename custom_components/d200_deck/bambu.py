"""Bambu Lab printers for the deck's plugin Bambu Lab.

The printers come from the Bambu Lab integration (domain `bambu_lab`). Its entities are found
by the end of their unique id (`<serial>_print_status`, ...), which does not change with the
language or when an entity is renamed. Each printer becomes one message for the deck:

    {"name": "P1S", "model": "p1s", "status": "printing", "progress": 34, "remainingMinutes": 96,
     "nozzle": 245, "nozzleTarget": 245, "bed": 70, "bedTarget": 70, "lightOn": true,
     "filamentColor": "#74B9FF", "layer": 102, "totalLayers": 300,
     "spools": [{"material": "PLA", "remaining": 80, "color": "#FFFFFF"}]}

`payload()` and `status()` are plain functions (tests/ha/test_bambu.py).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

BAMBU_DOMAIN = "bambu_lab"

# what the deck reads -> the end of the entity's unique id in the Bambu Lab integration
KEYS = {
    "status": "_print_status",
    "stage": "_stage",
    "progress": "_print_progress",
    "remaining": "_remaining_time",
    "nozzle": "_nozzle_temp",
    "nozzle_target": "_target_nozzle_temp",
    "bed": "_bed_temp",
    "bed_target": "_target_bed_temp",
    "light": "_chamber_light",
    "online": "_online",
    "error": "_print_error",
    "layer": "_current_layer",
    "total_layers": "_total_layers",
    "active_tray": "_active_tray",
}

# stages of a running print that are not printing yet
HEATING = {
    "heatbed_preheating", "heating_hotend", "heating_chamber", "waiting_for_heatbed_temperature", "thermal_preconditioning",
    "preparing_hotend", "checking_extruder_temperature", "waiting_chamber_temperature_equalize", "heatbed_stabilizing",
}
CALIBRATING = {
    "auto_bed_leveling", "bed_level_phase_1", "bed_level_phase_2", "bed_level_high_temperature", "homing_toolhead", "scanning_bed_surface",
    "sweeping_xy_mech_mode", "cleaning_nozzle_tip", "calibrating_extrusion", "calibrating_extrusion_flow", "calibrating_micro_lidar",
    "calibrating_motor_noise", "calibrate_nozzle_offset", "calibrating_camera_offset", "identifying_build_plate_type", "check_material",
    "check_material_position", "build_plate_alignment_detection", "absolute_accuracy_calibration", "laser_calibration", "preparing_ams",
    "filament_loading", "filament_unloading", "changing_filament", "print_calibration_lines", "pre_extrusion_before_printing",
    "moving_toolhead_to_center_of_heatbed", "measuring_surface", "heatbed_surface_foreign_object_detection",
}
UNKNOWN = (None, "", "unknown", "unavailable", "none")


@dataclass
class Printer:
    """One printer: its device and where its values are."""

    device_id: str
    name: str
    model: str
    entities: dict[str, str] = field(default_factory=dict)  # key of KEYS -> entity id
    trays: list[str] = field(default_factory=list)  # AMS slots in order
    external: str | None = None  # the external spool

    def watched(self) -> list[str]:
        return [*self.entities.values(), *self.trays, *([self.external] if self.external else [])]


def model_of(model: str | None) -> str:
    """"A1MINI" -> "a1-mini", "A1" -> "a1"; every enclosed printer is drawn as the P1S."""
    m = "".join(c for c in (model or "").lower() if c.isalnum())
    return {"a1mini": "a1-mini", "a1": "a1"}.get(m, "p1s")


def _num(value: Any) -> float | None:
    if value in UNKNOWN:
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _colour(value: Any) -> str | None:
    """"#RRGGBBAA" (the integration's) -> "#RRGGBB"."""
    if not isinstance(value, str) or not value.startswith("#") or len(value) not in (7, 9):
        return None
    return value[:7].upper()


def status(print_status: str | None, stage: str | None, online: str | None, error: str | None) -> str:
    """The deck's state (idle, heating, calibrating, printing, paused, complete, error, offline)."""
    if online == "off" or print_status in (*UNKNOWN, "offline"):
        return "offline"
    if print_status == "failed" or (error == "on" and print_status in ("running", "pause")):
        return "error"
    if print_status == "finish":
        return "complete"
    if print_status == "pause":
        return "paused"
    if print_status in ("prepare", "slicing", "init"):
        return "calibrating"
    if print_status == "running":
        if stage in HEATING:
            return "heating"
        if stage in CALIBRATING:
            return "calibrating"
        if stage and stage.startswith("paused_"):
            return "paused"
        return "printing"
    return "idle"


def _spool(state: str | None, attrs: dict[str, Any]) -> dict[str, Any] | None:
    """A slot of the AMS (or the external spool) for the filament view; None when it is empty."""
    if state in UNKNOWN or attrs.get("empty"):
        return None
    remain = _num(attrs.get("remain"))
    return {
        "material": attrs.get("type") or state,
        "remaining": remain if attrs.get("remain_enabled", True) and remain is not None and remain >= 0 else None,
        "color": _colour(attrs.get("color")),
    }


def payload(name: str, model: str, values: dict[str, tuple[str | None, dict[str, Any]]],
            trays: list[tuple[str | None, dict[str, Any]]], external: tuple[str | None, dict[str, Any]] | None) -> dict[str, Any]:
    """The message for the deck. `values`: key of KEYS -> (state, attributes)."""
    def st(key: str) -> str | None:
        return values.get(key, (None, {}))[0]

    remaining = _num(st("remaining"))
    if remaining is not None:
        unit = values["remaining"][1].get("unit_of_measurement")
        remaining = remaining * 60 if unit == "h" else remaining / 60 if unit == "s" else remaining
    light = st("light")
    slots = [(s, a) for s, a in [*trays, *([external] if external else [])]]
    # the loaded filament: the slot the integration marks active, else its "active tray" sensor
    active = next((a for s, a in slots if a.get("active") and s not in UNKNOWN), None) or values.get("active_tray", (None, {}))[1]
    spools = [sp for sp in (_spool(s, a) for s, a in trays) if sp]
    if not spools and external and (sp := _spool(*external)):
        spools = [sp]
    out = {
        "name": name,
        "model": model_of(model),
        "status": status(st("status"), st("stage"), st("online"), st("error")),
        "progress": _num(st("progress")),
        "remainingMinutes": round(remaining) if remaining is not None else None,
        "nozzle": _num(st("nozzle")),
        "nozzleTarget": _num(st("nozzle_target")),
        "bed": _num(st("bed")),
        "bedTarget": _num(st("bed_target")),
        "lightOn": True if light == "on" else False if light == "off" else None,
        "filamentColor": _colour(active.get("color")),
        "layer": _num(st("layer")),
        "totalLayers": _num(st("total_layers")),
        "spools": spools[:4],
    }
    return {k: v for k, v in out.items() if v is not None}


# ------------------------------------------------------------------ Home Assistant


def find_printers(hass) -> list[Printer]:
    """The printers of the Bambu Lab integration, with their AMS slots and external spool."""
    from homeassistant.helpers import device_registry as dr, entity_registry as er

    devs, ents = dr.async_get(hass), er.async_get(hass)
    entries = {e.entry_id for e in hass.config_entries.async_entries(BAMBU_DOMAIN)}
    if not entries:
        return []
    devices = [d for d in devs.devices.values() if d.config_entries & entries]
    out = []
    for dev in devices:
        if dev.via_device_id is not None:
            continue
        mine = [e for e in er.async_entries_for_device(ents, dev.id, include_disabled_entities=False) if e.platform == BAMBU_DOMAIN]
        found = {key: e.entity_id for key, end in KEYS.items() for e in mine if e.unique_id.endswith(end)}
        if "status" not in found:
            continue  # not a printer (the MakerWorld profile)
        p = Printer(dev.id, dev.name_by_user or dev.name or "Printer", dev.model or "", found)
        for child in sorted((d for d in devices if d.via_device_id == dev.id), key=lambda d: d.name or ""):
            for e in sorted(er.async_entries_for_device(ents, child.id, include_disabled_entities=False), key=lambda e: e.unique_id):
                if e.platform != BAMBU_DOMAIN or e.domain != "sensor":
                    continue
                if e.unique_id.endswith("_external_spool"):
                    p.external = e.entity_id
                elif "_tray_" in e.unique_id and e.unique_id.rsplit("_", 1)[-1].isdigit():
                    p.trays.append(e.entity_id)
        out.append(p)
    return sorted(out, key=lambda p: p.name)


def printer_payload(hass, p: Printer) -> dict[str, Any]:
    def read(entity_id: str | None) -> tuple[str | None, dict[str, Any]]:
        st = hass.states.get(entity_id) if entity_id else None
        return (st.state, dict(st.attributes)) if st else (None, {})

    return payload(p.name, p.model, {k: read(e) for k, e in p.entities.items()}, [read(t) for t in p.trays], read(p.external) if p.external else None)
