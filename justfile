set dotenv-load

# Show available commands
_default:
    @just --list --justfile {{ justfile() }}

# build and install python package
[group('build')]
build-py:
    uvx --from 'maturin[zig]' maturin develop -m python/Cargo.toml

# build the python package into python/.venv and run its tests
[group('check')]
test-py:
    uv venv -q --allow-existing python/.venv
    uv pip install -q --python python/.venv pytest pyarrow polars
    VIRTUAL_ENV="$PWD/python/.venv" uvx --from 'maturin[zig]' maturin develop -q -m python/Cargo.toml
    python/.venv/bin/python -m pytest python/tests -q

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

# apply clippy auto-fixes, then format (clippy may rewrite code fmt must tidy)
[group('dev')]
fix:
    cargo clippy --workspace --all-targets --all-features --fix --allow-dirty
    cargo fmt --all

# lint gate, as in CI: fmt check, clippy -D warnings, machete, ruff, typos
[group('check')]
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    cargo machete
    uvx ruff check .
    uvx ruff format --check .
    typos

# run the test suite, as in CI: nextest plus doctests
[group('check')]
test *args:
    cargo nextest run --workspace --all-features --locked {{ args }}
    cargo test --workspace --all-features --locked --doc

# build rustdoc with warnings denied, as in CI
[group('check')]
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked

# check the workspace builds on the declared MSRV (rust-version in Cargo.toml)
[group('check')]
msrv:
    cargo +1.95 check --workspace --all-targets --all-features --locked

# everything CI gates on, in one go (run before pushing)
[group('check')]
check: lint test doc
