"""Constants of the D200 Deck integration."""

DOMAIN = "d200_deck"

CONF_BASE_TOPIC = "base_topic"
CONF_ENTITIES = "entities"
CONF_DEFAULT_ACTIONS = "default_actions"

DEFAULT_BASE_TOPIC = "d200"

# Fired for every press on a key that shows an entity of this integration.
EVENT_PRESS = "d200_deck_press"

SERVICE_NOTIFY = "notify"
SERVICE_SET_KEY = "set_key"
SERVICE_CLEAR_KEY = "clear_key"
SERVICE_SHOW_REMOTE = "show_remote_page"
