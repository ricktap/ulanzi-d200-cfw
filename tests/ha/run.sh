#!/bin/sh
# Tests of the Home Assistant integration: the translation alone and the integration in
# Home Assistant's test harness. Needs: pip install pytest-homeassistant-custom-component
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
${PYTHON:-python3} -m pytest -q -p no:cacheprovider -o asyncio_mode=auto tests/ha "$@"
