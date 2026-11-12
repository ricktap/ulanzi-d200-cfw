"""Constants of the D200 Deck integration."""

DOMAIN = "d200_deck"

# How Home Assistant talks to D200 Deck: MQTT, the deck's Web API, or both.
CONF_TRANSPORT = "transport"
TRANSPORT_MQTT = "mqtt"
TRANSPORT_WEB = "web"
TRANSPORT_BOTH = "both"

CONF_BASE_TOPIC = "base_topic"
CONF_HOST = "host"
CONF_PORT = "port"
CONF_TOKEN = "token"
CONF_WEBHOOK_ID = "webhook_id"
CONF_ENTITIES = "entities"
CONF_DEFAULT_ACTIONS = "default_actions"

DEFAULT_BASE_TOPIC = "d200"
DEFAULT_PORT = 8765

# Fired for every press on a key that shows an entity of this integration.
EVENT_PRESS = "d200_deck_press"

SERVICE_NOTIFY = "notify"
SERVICE_SET_KEY = "set_key"
SERVICE_CLEAR_KEY = "clear_key"
SERVICE_SHOW_REMOTE = "show_remote_page"
