# tests/load/Makefile
# Load test entry point — CI and local both call these targets

-include .env

REMOTE_HOST ?=
REMOTE_USER ?=
SERVER_BIN_PATH ?=
SSH_KEY     ?=
SERVER_PORT ?= 7985
REMOTE_DIR  ?= /tmp/memory-seek-server/loadtest

RESULTS_DIR  := results
BASELINES    := baselines
BASE_URL     := http://$(REMOTE_HOST):$(SERVER_PORT)

.PHONY: deploy teardown auth user photo social mixed all p95-report baseline compare clean help

# ── 部署 ──────────────────────────────────────────
deploy:                    ## 部署 infra + server 到远程
	bash setup/deploy.sh --host $(REMOTE_HOST) --user $(REMOTE_USER) \
		$(if $(SERVER_BIN_PATH),--server-bin-path $(SERVER_BIN_PATH),) \
		$(if $(SSH_KEY),--ssh-key $(SSH_KEY),) \
		--port $(SERVER_PORT) \
		--remote-dir $(REMOTE_DIR)

teardown:                  ## 清理远程环境（server + infra + 文件）
	bash setup/teardown.sh --host $(REMOTE_HOST) --user $(REMOTE_USER) \
		$(if $(SSH_KEY),--ssh-key $(SSH_KEY),) \
		--remote-dir $(REMOTE_DIR)

# ── 压测 ──────────────────────────────────────────
$(RESULTS_DIR):
	mkdir -p $(RESULTS_DIR)

auth: $(RESULTS_DIR)       ## 认证模块压测
	k6 run \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(RESULTS_DIR)/auth_summary.json \
		scenarios/auth.js

user: $(RESULTS_DIR)       ## 用户模块压测
	k6 run \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(RESULTS_DIR)/user_summary.json \
		scenarios/user.js

photo: $(RESULTS_DIR)      ## 照片模块压测
	k6 run \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(RESULTS_DIR)/photo_summary.json \
		scenarios/photo.js

social: $(RESULTS_DIR)     ## 社交模块压测（收藏夹 + 评论）
	k6 run \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(RESULTS_DIR)/social_summary.json \
		scenarios/social.js

mixed: $(RESULTS_DIR)      ## 混合场景压测
	k6 run \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(RESULTS_DIR)/mixed_summary.json \
		scenarios/mixed.js

all: auth user photo social mixed  ## 运行全部压测

# ── 报告 ──────────────────────────────────────────
p95-report:                ## 生成 P95 性能报告
	@echo "=== P95 Latency ==="
	@for f in $(RESULTS_DIR)/*_summary.json; do \
		name=$$(basename $$f _summary.json); \
		p95=$$(cat $$f | jq -r '.metrics.http_req_duration.values["p(95)"]' 2>/dev/null); \
		if [ "$$p95" != "null" ] && [ -n "$$p95" ]; then \
			printf "  %-15s %.2f ms\n" "$$name" "$$p95"; \
		fi; \
	done

baseline:                  ## 保存当前结果为基线
	@mkdir -p $(BASELINES)
	cp $(RESULTS_DIR)/*_summary.json $(BASELINES)/ 2>/dev/null || echo "No results to save"

compare:                   ## 对比当前结果与基线
	@echo "=== P95 Comparison ==="
	@printf "  %-15s %12s %12s %10s\n" "Endpoint" "Baseline" "Current" "Change"
	@printf "  %-15s %12s %12s %10s\n" "--------" "--------" "-------" "------"
	@for f in $(BASELINES)/*_summary.json; do \
		name=$$(basename $$f _summary.json); \
		base=$$(cat $$f | jq -r '.metrics.http_req_duration.values["p(95)"]' 2>/dev/null); \
		curr=$$(cat $(RESULTS_DIR)/$${name}_summary.json 2>/dev/null | jq -r '.metrics.http_req_duration.values["p(95)"]' 2>/dev/null); \
		if [ "$$base" != "null" ] && [ "$$curr" != "null" ] && [ -n "$$base" ] && [ -n "$$curr" ]; then \
			change=$$(echo "$$curr $$base" | awk '{printf "%.1f%%", ($$1/$$2 - 1) * 100}'); \
			printf "  %-15s %10s ms %10s ms %10s\n" "$$name" "$$base" "$$curr" "$$change"; \
		fi; \
	done

# ── 清理 ──────────────────────────────────────────
clean:                     ## 清理本地结果文件
	rm -rf $(RESULTS_DIR)/*.json $(BASELINES)/*.json

help:                      ## 显示帮助
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*##"} {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}' | sort
