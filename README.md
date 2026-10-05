# kestrel-telemetry

Telemetry ingest for the Kestrel medical drone delivery platform.

Drones publish MAVLink over MQTT. This service decodes each frame, asks
dispatch which mission the drone is flying, publishes the enriched frame to
Kafka, and keeps a short in-memory window per drone so the console's live map
can poll an internal query API.

## Where it fits

- Upstream: drones via the MQTT broker
- Calls: kestrel-dispatch (`/internal/v1/drones/{id}/active-mission`)
- Downstream: Kafka topic `drone.telemetry.v2`, and kestrel-gateway / console via the query API
- Schemas come from kestrel-contracts (`TelemetryFrame`, `Mission`)

## Local development

```
cargo test
KESTREL_ENV=development cargo run
```

Settings load from `config/default.toml`, then `config/$KESTREL_ENV.toml`, then
`KESTREL__SECTION__KEY` environment variables, e.g. `KESTREL__API__BIND=127.0.0.1:9191`.

Query API: `GET /v1/drones`, `GET /v1/drones/:id/track`, `GET /healthz`.

## Deploy

CI builds `ghcr.io/roykaushik07/kestrel-telemetry:<sha>` on every push to main.
Releases are tagged by hand and rolled out from `deploy/k8s`.
