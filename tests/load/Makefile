# tests/load/Makefile
# 端到端压测自动化入口

CONFIG_FILE ?= config/remote.json
BASE_URL    ?= $(shell jq -r .baseUrl $(CONFIG_FILE))
SSH_HOST    ?= $(shell jq -r .ssh.host $(CONFIG_FILE))
SSH_PORT    ?= $(shell jq -r .ssh.port $(CONFIG_FILE))
SSH_USER    ?= $(shell jq -r .ssh.user $(CONFIG_FILE))
SSH_KEY     ?= $(shell jq -r .ssh.keyFile $(CONFIG_FILE))
DB_NAME     ?= $(shell jq -r .db.database $(CONFIG_FILE))
DB_USER     ?= $(shell jq -r .db.user $(CONFIG_FILE))

SSH_CMD = ssh -i $(SSH_KEY) -p $(SSH_PORT) $(SSH_USER)@$(SSH_HOST)
PSQL_CMD = $(SSH_CMD) "psql -U $(DB_USER) -d $(DB_NAME)"

.PHONY: seed cleanup verify loadtest loadtest-auth loadtest-user loadtest-photo help

help: ## 显示帮助信息
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

seed: ## 预置压测数据
	@echo "📦 预置压测数据..."
	$(PSQL_CMD) < setup/seed.sql
	@echo "✅ 数据预置完成"

cleanup: ## 清理压测数据
	@echo "🧹 清理压测数据..."
	$(PSQL_CMD) < setup/cleanup.sql
	@echo "✅ 数据清理完成"

verify: ## 验证压测数据
	@echo "🔍 验证压测数据..."
	$(PSQL_CMD) < setup/verify.sql

loadtest: seed ## 全量压测 (auth + user + photo)
	@echo "🚀 开始全量压测..."
	k6 run -e BASE_URL=$(BASE_URL) scripts/auth.js
	k6 run -e BASE_URL=$(BASE_URL) scripts/user.js
	k6 run -e BASE_URL=$(BASE_URL) scripts/photo.js
	@$(MAKE) cleanup
	@echo "✅ 全量压测完成"

loadtest-auth: seed ## 仅认证模块压测
	@echo "🚀 开始认证模块压测..."
	k6 run -e BASE_URL=$(BASE_URL) scripts/auth.js
	@$(MAKE) cleanup
	@echo "✅ 认证模块压测完成"

loadtest-user: seed ## 仅用户模块压测
	@echo "🚀 开始用户模块压测..."
	k6 run -e BASE_URL=$(BASE_URL) scripts/user.js
	@$(MAKE) cleanup
	@echo "✅ 用户模块压测完成"

loadtest-photo: seed ## 仅照片模块压测
	@echo "🚀 开始照片模块压测..."
	k6 run -e BASE_URL=$(BASE_URL) scripts/photo.js
	@$(MAKE) cleanup
	@echo "✅ 照片模块压测完成"

loadtest-quick: ## 快速压测 (不预置数据，不清理)
	@echo "⚡ 开始快速压测..."
	k6 run -e BASE_URL=$(BASE_URL) --duration 30s --vus 10 scripts/auth.js
	@echo "✅ 快速压测完成"
