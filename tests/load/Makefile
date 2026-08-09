# tests/load/Makefile
# Load test entry point — run k6 scenarios against a running server
#
# 压测体系分层:
#   编排层   : ci-up / ci-down / run-all / run-once / photo-nos3 / seed
#   采集层   : collect / prometheus
#   判定层   : judge / record-baseline
#   报告层   : report
#   原有层   : auth / user / photo / p95-report / baseline / compare

REMOTE_HOST ?= 127.0.0.1
SERVER_PORT ?= 7985
METRICS_PORT ?= 9090

# 构建与运行
FEATURES ?= metrics,auth,user,photo,face-engine
BIN_NAME  ?= memory-seek-server
RUNS      ?= 3          # 每场景轮次, collect 取中位数
INCLUDE_S3_PATHS ?= false

# photo 场景中不依赖 S3 的子场景(独立脚本文件名, 对应 scenarios/photo/<name>.js)
PHOTO_NOS3_SERVICES ?= collection collection_photo comment comment_like

# face/person 压测数据量(与 seed.sql 对齐)
PHOTO_COUNT ?= 40000   # photo_photo 总数(= PHOTO_USERS * PHOTOS_PER_USER)
FACES        ?= 40000  # photo_face 总行数(= 每照片 1 张脸)
PERSONS      ?= 2000   # photo_person 总行数(= 每 photo 用户 1 个)

# ── 负载模型 ─────────────────────────────────────────
# LOAD_MODE=target(默认): constant-arrival-rate 固定迭代率, 稳定对比
# LOAD_MODE=max:          ramping-arrival-rate 逐步加压, MAX_RPS 为极大软目标
#                         (rate 实际由系统饱和/VU 上限决定, 即"无上限找上限")
LOAD_MODE    ?= target
# 各场景目标/上限迭代率(单 op 场景即 QPS; photo 为完整流程迭代率, QPS≈迭代率×请求数)
AUTH_TARGET_RPS   ?= 1500
AUTH_MAX_RPS      ?= 100000
USER_TARGET_RPS   ?= 500
USER_MAX_RPS      ?= 100000
FACE_TARGET_RPS   ?= 500
FACE_MAX_RPS      ?= 100000
PERSON_TARGET_RPS ?= 500
PERSON_MAX_RPS    ?= 100000
PHOTO_TARGET_RPS  ?= 30
PHOTO_MAX_RPS     ?= 100000
# 压测时长(target 模式, 如 2m/3m; max 模式 stages 固定 1m+2m+1m)
DURATION ?= 2m

RESULTS_DIR  := results
BASELINES    := baselines
CI_DIR       := ci
SERVER_BIN   := ../../target/release/$(BIN_NAME)
SERVER_LOG   := $(CI_DIR)/server.log
SERVER_PID   := $(CI_DIR)/server.pid
COMPOSE      := docker compose -f docker-compose.load.yml
BASE_URL     := http://$(REMOTE_HOST):$(SERVER_PORT)

# 多轮输出目录: 指定 RUN 时写入 run-<n>, 否则沿用原路径
ifeq ($(strip $(RUN)),)
OUT_DIR := $(RESULTS_DIR)
else
OUT_DIR := $(RESULTS_DIR)/run-$(RUN)
endif

.PHONY: all p95-report baseline compare clean help \
        ci-up ci-down run-all run-once photo-nos3 seed collect prometheus judge record-baseline report \
        face person

# ── 压测(原有) ──────────────────────────────────────
$(OUT_DIR):
	mkdir -p $@

auth: $(OUT_DIR)       ## 认证模块压测
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e LOAD_MODE=$(LOAD_MODE) \
		-e TARGET_RPS=$(AUTH_TARGET_RPS) \
		-e MAX_RPS=$(AUTH_MAX_RPS) \
		-e DURATION=$(DURATION) \
		-e SUMMARY_EXPORT=$(OUT_DIR)/auth_summary.json \
		scenarios/auth.js

user: $(OUT_DIR)       ## 用户模块压测
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e LOAD_MODE=$(LOAD_MODE) \
		-e TARGET_RPS=$(USER_TARGET_RPS) \
		-e MAX_RPS=$(USER_MAX_RPS) \
		-e DURATION=$(DURATION) \
		-e SUMMARY_EXPORT=$(OUT_DIR)/user_summary.json \
		scenarios/user.js

photo: $(OUT_DIR)      ## 照片模块压测(完整 photo.js, 含 S3 场景)
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e SUMMARY_EXPORT=$(OUT_DIR)/photo_summary.json \
		scenarios/photo.js

face: $(OUT_DIR)       ## 人脸模块压测(face_controller)
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e LOAD_MODE=$(LOAD_MODE) \
		-e TARGET_RPS=$(FACE_TARGET_RPS) \
		-e MAX_RPS=$(FACE_MAX_RPS) \
		-e DURATION=$(DURATION) \
		-e PHOTO_COUNT=$(PHOTO_COUNT) \
		-e FACES=$(FACES) \
		-e PERSONS=$(PERSONS) \
		-e SUMMARY_EXPORT=$(OUT_DIR)/face_summary.json \
		scenarios/photo/face.js

person: $(OUT_DIR)     ## 人物模块压测(person_controller)
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e LOAD_MODE=$(LOAD_MODE) \
		-e TARGET_RPS=$(PERSON_TARGET_RPS) \
		-e MAX_RPS=$(PERSON_MAX_RPS) \
		-e DURATION=$(DURATION) \
		-e PERSONS=$(PERSONS) \
		-e SUMMARY_EXPORT=$(OUT_DIR)/person_summary.json \
		scenarios/photo/person.js

all: auth user photo face person  ## 运行全部压测(含 S3 场景与 face/person)

# ── 环境编排 ────────────────────────────────────────
ci-up:                 ## 启动压测环境(容器依赖 + release server 进程)
	$(COMPOSE) up -d --wait
	cd ../.. && cargo build --release --features "$(FEATURES)"
	@echo "Starting server (features: $(FEATURES))..."
	@cd ../.. && ./target/release/$(BIN_NAME) --config $(CI_DIR)/server-config.yml \
		--log-dir $(CI_DIR)/logs \
		> $(CI_DIR)/server.log 2>&1 & echo $$! > $(SERVER_PID)
	@for i in $$(seq 1 60); do \
		if curl -sf http://127.0.0.1:$(SERVER_PORT)/health > /dev/null; then \
			echo "Server ready at $(BASE_URL)"; exit 0; \
		fi; \
		sleep 1; \
	done; \
	echo "ERROR: server failed to start"; tail -50 $(SERVER_LOG); exit 1

ci-down:               ## 停止压测环境(server 进程 + 容器)
	@if [ -f $(SERVER_PID) ]; then \
		kill $$(cat $(SERVER_PID)) 2>/dev/null || true; \
		rm -f $(SERVER_PID); \
	fi
	$(COMPOSE) down

# ── 压测(编排层) ────────────────────────────────────
run-all:               ## 全场景压测(默认排除 S3 路径), 多轮取中位数
	@for run in $$(seq 1 $(RUNS)); do \
		echo "===== Run $$run / $(RUNS) ====="; \
		$(MAKE) run-once RUN=$$run; \
		sleep 5; \
	done
	@$(MAKE) collect

run-once:              ## 单轮全场景压测(auth + user + photo + face + person)
	$(MAKE) auth RUN=$(RUN) || echo "[warn] auth 轮 $(RUN) 失败(见 results)"
	$(MAKE) user RUN=$(RUN) || echo "[warn] user 轮 $(RUN) 失败(见 results)"
	$(MAKE) photo-nos3 RUN=$(RUN) || echo "[warn] photo 轮 $(RUN) 失败(见 results)"
	$(MAKE) face RUN=$(RUN) || echo "[warn] face 轮 $(RUN) 失败(见 results)"
	$(MAKE) person RUN=$(RUN) || echo "[warn] person 轮 $(RUN) 失败(见 results)"

photo-nos3: $(OUT_DIR) ## photo 压测, 仅跑不依赖 S3 的子场景(直接运行独立脚本)
	@for s in $(PHOTO_NOS3_SERVICES); do \
		echo "=== photo scenario: $$s ==="; \
		k6 run -q \
			-e BASE_URL=$(BASE_URL) \
			-e LOAD_MODE=$(LOAD_MODE) \
			-e TARGET_RPS=$(PHOTO_TARGET_RPS) \
			-e MAX_RPS=$(PHOTO_MAX_RPS) \
			-e DURATION=$(DURATION) \
			-e SUMMARY_EXPORT=$(OUT_DIR)/photo_$${s}_summary.json \
			scenarios/photo/$$s.js || echo "[warn] scenario $$s 失败"; \
	done

seed:                  ## 灌入种子数据(建表 + 压测账号/照片/时间线统计)
	$(CI_DIR)/../seed/seed.sh

# ── 采集层 ──────────────────────────────────────────
collect:               ## 多轮结果归一化(逐指标取中位数) -> results/normalized/
	$(CI_DIR)/collect.sh

prometheus:            ## 抓取服务端 metrics(9090)快照 -> results/metrics/
	$(CI_DIR)/prometheus.sh

# ── 判定层 ──────────────────────────────────────────
judge:                 ## 当前结果对比基线, 按 thresholds.yml 判定
	$(CI_DIR)/judge.sh

record-baseline:       ## 将当前 normalized 结果记录为基线
	@mkdir -p $(BASELINES)
	@cp -n $(RESULTS_DIR)/normalized/*.json $(BASELINES)/ 2>/dev/null || \
		echo "[record-baseline] 未发现 normalized 结果, 请先 make collect"

# ── 报告层 ──────────────────────────────────────────
report:                ## 生成 Markdown 对比报告 -> results/report.md
	$(CI_DIR)/report.sh

# ── 报告(原有) ──────────────────────────────────────
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
	rm -rf $(RESULTS_DIR)/*.json $(RESULTS_DIR)/run-* $(BASELINES)/*.json

help:                      ## 显示帮助
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*##"} {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}' | sort
