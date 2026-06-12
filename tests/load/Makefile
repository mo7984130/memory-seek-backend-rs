# tests/load/Makefile
# NOTE: Run from repo root with: make -f tests/load/Makefile <target>

REPO_ROOT := $(shell cd ../.. && pwd)

.PHONY: test-unit test-integration test-load test-all clean

test-unit:
	cd $(REPO_ROOT) && cargo test --lib

test-integration:
	podman compose up -d postgres redis minio
	sleep 10
	@echo "初始化测试数据库..."
	@podman exec load_postgres_1 psql -U test -d memory_seek_test -c 'CREATE TABLE IF NOT EXISTS "auth_user" (id BIGSERIAL PRIMARY KEY, username VARCHAR(255) NOT NULL, email VARCHAR(255) NOT NULL, password VARCHAR(255) NOT NULL, nickname VARCHAR(255) NOT NULL, avatar_file_id VARCHAR(2000), inviter BIGINT NOT NULL, refresh_token CHAR(32), refresh_token_expire_at TIMESTAMPTZ, updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL, created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL, CONSTRAINT user_username_index UNIQUE (username), CONSTRAINT user_email_index UNIQUE (email));' || true
	@podman exec load_redis_1 redis-cli FLUSHALL
	cd $(REPO_ROOT) && cargo test --test integration --features "auth" -- --test-threads=1; rc=$$?; exit $$rc

test-load: test-load-auth test-load-photo

test-load-auth:
	podman compose --profile load up -d
	sleep 30
	k6 run scripts/auth.js --out json=results/auth.json
	podman compose --profile load down -v

test-load-photo:
	podman compose --profile load up -d
	sleep 30
	k6 run scripts/photo.js --out json=results/photo.json
	podman compose --profile load down -v

test-all: test-unit test-integration test-load

clean:
	podman compose --profile load down -v
	rm -rf results/*.json results/*.csv
