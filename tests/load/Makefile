# tests/load/Makefile

.PHONY: test-unit test-integration test-load test-all clean

test-unit:
	cargo test --lib

test-integration:
	docker compose up -d postgres redis minio
	sleep 10
	cargo test --test integration --features "auth,user,photo"
	docker compose down -v

test-load: test-load-auth test-load-photo

test-load-auth:
	docker compose --profile load up -d
	sleep 30
	k6 run scripts/auth.js --out json=results/auth.json
	docker compose --profile load down -v

test-load-photo:
	docker compose --profile load up -d
	sleep 30
	k6 run scripts/photo.js --out json=results/photo.json
	docker compose --profile load down -v

test-all: test-unit test-integration test-load

clean:
	docker compose --profile load down -v
	rm -rf results/*.json results/*.csv
