# tests/load/Makefile
# 隔离式压测自动化入口

CONFIG_FILE ?= config/remote.json

# 从配置文件读取连接信息
SSH_HOST    ?= $(shell jq -r .ssh.host $(CONFIG_FILE))
SSH_PORT    ?= $(shell jq -r .ssh.port $(CONFIG_FILE))
SSH_USER    ?= $(shell jq -r .ssh.user $(CONFIG_FILE))
DB_USER     ?= $(shell jq -r .db.user $(CONFIG_FILE))
DB_PASS     ?= $(shell jq -r .db.pass $(CONFIG_FILE))
SERVER_BIN  ?= $(shell jq -r .server_bin $(CONFIG_FILE))

# 数据量配置
AUTH_USERS  ?= 10000
PHOTO_USERS ?= 20
PHOTOS      ?= 100000

# k6 目标服务器（临时服务端口）
BASE_URL    ?= http://$(SSH_HOST):7985

SSH_CMD = ssh -p $(SSH_PORT) $(SSH_USER)@$(SSH_HOST)

# 脚本路径（相对于 tests/load/）
SETUP_DIR = setup

.PHONY: help seed start-server stop-server teardown loadtest loadtest-quick verify

help: ## 显示帮助信息
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

seed: ## SSH 创建数据库 + 建表 + 填充数据
	@echo "=== Seed: 创建压测数据库 ==="
	$(SSH_CMD) "bash -s" < $(SETUP_DIR)/start-loadtest.sh \
		$(DB_USER) $(DB_PASS) $(AUTH_USERS) $(PHOTO_USERS) $(PHOTOS) \
		$(SERVER_BIN)

stop-server: ## SSH 停止临时服务 + 删库 + 清理配置
	@echo "=== Teardown: 清理压测环境 ==="
	$(SSH_CMD) "bash -s" < $(SETUP_DIR)/stop-loadtest.sh $(DB_USER) $(DB_PASS)

teardown: stop-server ## 同 stop-server

loadtest: seed ## 完整压测流程 (seed → k6 run all → teardown)
	@echo "=== Loadtest: 开始全量压测 ==="
	@trap 'make teardown' EXIT; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/auth/auth_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/user/user_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/photo/photo_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/photo/comment_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/photo/collection_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/photo/collection_photo_service.js
	@echo "=== Loadtest: 全量压测完成 ==="

loadtest-quick: ## 快速压测 (少量数据, 仅 auth 模块)
	@echo "=== Quick Loadtest ==="
	@AUTH_USERS=100 PHOTO_USERS=5 PHOTOS=100 make seed
	@trap 'make teardown' EXIT; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=100 -e PHOTO_USERS=5 \
		--duration 30s --vus 10 \
		scripts/auth/auth_service.js
	@echo "=== Quick Loadtest 完成 ==="

verify: ## 验证压测数据量
	@echo "=== 验证压测数据 ==="
	$(SSH_CMD) "PGPASSWORD=$(DB_PASS) psql -h localhost -p 5433 -U $(DB_USER) -d memory_seek_loadtest" \
		< $(SETUP_DIR)/verify.sql
