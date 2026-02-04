.PHONY: all build run check test lint fmt clean dev

# Default target
all: check

# Build the project
build:
	cargo build

# Run the game
run:
	cargo run

# Run with dynamic linking (faster iteration)
# Note: bevy_dylib 0.18.0 not yet on crates.io - use 'make run' until available
dev:
	cargo run --features bevy/dynamic_linking

# Run all checks (lint + test)
check: lint test

# Run tests
test:
	cargo test

# Run unit tests only
test-unit:
	cargo test --lib

# Run integration tests only
test-integration:
	cargo test --test integration

# Run e2e tests only
test-e2e:
	cargo test --test e2e

# Run linting
lint: fmt-check clippy

# Check formatting
fmt-check:
	cargo fmt --check

# Apply formatting
fmt:
	cargo fmt

# Run clippy
clippy:
	cargo clippy -- -D warnings

# Clean build artifacts
clean:
	cargo clean

# Build release version
release:
	cargo build --release

# Run release version
run-release:
	cargo run --release
