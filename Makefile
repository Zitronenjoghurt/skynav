.PHONY: up down build dev check app web server fmt

up:
	docker compose -f docker/docker-compose.yml up -d

down:
	docker compose -f docker/docker-compose.yml down

build:
	docker image prune -f
	docker compose -f docker/docker-compose.yml build

dev:
	docker compose -f docker/docker-compose.yml --profile dev up server-dev

fmt:
	cargo fmt --all

check:
	cargo fmt --all --check
	cargo clippy -p skynav --all-targets -- -D warnings
	cargo clippy -p skynav-server --all-targets -- -D warnings
	cargo clippy -p skynav-app --all-targets -- -D warnings
	cargo clippy -p skynav-app --target wasm32-unknown-unknown -- -D warnings
	cargo test -p skynav
	cargo build -p skynav-app --target wasm32-unknown-unknown

app:
	cargo run -p skynav-app

web:
	trunk serve --open

server:
	cargo run -p skynav-server
