# Running Holochain with OpenTelemetry



You can ignore the config files in this directory, they're all loaded automatically. Start everything up with

```shell
docker compose up -d
```

or using `podman`

```
podman-compose up -d
```

Start Holochain with, setting the log level as you please

```shell
OTEL_EXPORT=otlp OTEL_SERVICE_NAME=holochain RUST_LOG=info holochain ...
```

Now log into Grafana at (http://localhost:3000)[http://localhost:3000] and log in with `admin/admin`.
