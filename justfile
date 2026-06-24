set dotenv-load := true

# Show available commands
_default:
    @just --list --justfile {{ justfile() }}

# run protobuf code generation
[group('build')]
generate:
    buf generate proto
    cargo clippy --fix --allow-dirty
    cargo fmt --all

# build and install python package
[group('build')]
build-py:
    uvx --from 'maturin[zig]' maturin develop -m python/Cargo.toml

# build python server bindings
[group('build')]
build-py-cli:
    uv run maturin develop --uv --manifest-path crates/cli/Cargo.toml

# run simulation for a given duration
[group('caspers')]
run duration='100':
    cargo run --bin caspers -- run --duration {{ duration }}

[group('caspers')]
run-tracing:
    docker run -d --name jaeger \
        -e COLLECTOR_ZIPKIN_HOST_PORT=:9411 \
        -e COLLECTOR_OTLP_ENABLED=true \
        -p 6831:6831/udp \
        -p 6832:6832/udp \
        -p 5778:5778 \
        -p 16686:16686 \
        -p 4317:4317 \
        -p 4318:4318 \
        -p 14250:14250 \
        -p 14268:14268 \
        -p 14269:14269 \
        -p 9411:9411 \
        jaegertracing/all-in-one:latest

# run marimo notebook server for interactive data exploration
[group('dev')]
scratch:
    uv run --directory {{ source_directory() }}/notebooks marimo edit explore.py

# run sim-kitchen simulation and write events to parquet
[group('caspers')]
sim-kitchen steps='100':
    cargo run --example basic -p sim-kitchen -- {{ steps }}

# inspect simulation events in marimo notebook
[group('dev')]
inspect:
    uv run --directory {{ source_directory() }}/notebooks marimo edit sim_events.py

# run interactive kitchen simulation notebook (run + analyze in one place)
[group('dev')]
kitchen:
    uv run --directory {{ source_directory() }}/notebooks marimo edit sim_kitchen.py

# format and linting (runs pre-commit-hooks)
[group('dev')]
fmt:
    uvx pre-commit run --all-files
    just --fmt --unstable

# run docs site dev server
[group('dev')]
docs:
    npm -w docs run dev

# build the UI for production
[group('server')]
build-ui:
    npm -w ui run build

[group('server')]
build-docs:
    npm -w docs run build

# run UI dev server (with API proxy)
[group('server')]
dev-ui:
    npm -w ui run dev

# install UI dependencies
[group('server')]
install-ui:
    npm -w ui install
