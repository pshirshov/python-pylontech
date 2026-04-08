# pylontech-mqtt-adapter

Standalone Rust MQTT adapter for the existing Pylontech TCP bridge workflow.

It does not modify or depend on the Python poller at runtime. It:

- connects directly to the battery bridge over TCP
- scans a configurable address range
- polls per-module battery values
- publishes Home Assistant MQTT discovery payloads
- publishes stack and per-module JSON state topics

## Usage

```bash
nix shell nixpkgs#cargo nixpkgs#rustc -c \
  cargo run --manifest-path rust-mqtt-adapter/Cargo.toml -- \
  192.168.1.7 \
  --mqtt-host mqtt.local \
  --mqtt-user mqtt \
  --mqtt-password-file /var/run/agenix/mqtt-user
```

## Topics

- Availability: `pylontech/status`
- Stack state: `pylontech/stack/state`
- Module state: `pylontech/module/<address>/state`
- Discovery: `homeassistant/sensor/.../config`

Both the topic prefix and discovery prefix are configurable.
