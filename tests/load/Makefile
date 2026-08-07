# tests/load/Makefile
# Load test entry point — run k6 scenarios against a running server

REMOTE_HOST ?=
SERVER_PORT ?= 7985

RESULTS_DIR  := results
BASELINES    := baselines
BASE_URL     := http://$(REMOTE_HOST):$(SERVER_PORT)

.PHONY: auth user photo all p95-report baseline compare clean help

# ── 压测 ──────────────────────────────────────────
$(RESULTS_DIR):
	mkdir -p $(RESULTS_DIR)

auth: $(RESULTS_DIR)       ## 认证模块压测
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(RESULTS_DIR)/auth_summary.json \
		scenarios/auth.js

user: $(RESULTS_DIR)       ## 用户模块压测
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(RESULTS_DIR)/user_summary.json \
		scenarios/user.js

photo: $(RESULTS_DIR)      ## 照片模块压测（可选 SERVICE=<name> 指定单个服务）
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(RESULTS_DIR)/photo_summary.json \
		$(if $(SERVICE),--scenario $(SERVICE),) \
		scenarios/photo.js

all: auth user photo  ## 运行全部压测

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
