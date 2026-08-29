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
# 完整目标地址优先，可用于 HTTPS、非默认端口和反向代理路径前缀。
BASE_URL ?= http://$(REMOTE_HOST):$(SERVER_PORT)

# 构建与运行
FEATURES ?= metrics,auth,user,photo,face-engine
BIN_NAME  ?= memory-seek-server
RUNS      ?= 3          # 每场景轮次, collect 取中位数
INCLUDE_S3_PATHS ?= false
SERVER_CPUS ?= 2        # 被测 server 占用核数(taskset 绑定, 控制变量)

# photo 场景中不依赖 S3 的子场景(独立脚本文件名, 对应 scenarios/photo/<name>.js)
PHOTO_NOS3_SERVICES ?= collection collection_photo comment comment_like photo_like

# face/person 压测数据量(与 seed.sql 对齐)
PHOTO_COUNT ?= 40000   # photo_photo 总数(= PHOTO_USERS * PHOTOS_PER_USER)
PHOTO_USERS ?= 2000    # photo 压测用户数(seed.sql 同值)
FACES        ?= 40000  # photo_face 总行数(= 每照片 1 张脸)
PERSONS      ?= 2000   # photo_person 总行数(= 每 photo 用户 1 个)
PHOTOS_PER_USER ?= 20  # 每 photo 用户照片数(seed.sql 同值; 供脚本按 VU 定位照片)

# ── 负载模型 ─────────────────────────────────────────
# target 使用固定到达率；max-adaptive 由 ci/max_adaptive.sh 按 SLO 逐档搜索容量。
# 各场景目标/容量搜索上限迭代率(单 op 场景即 QPS; photo 为完整流程迭代率)
AUTH_TARGET_RPS   ?= 400
AUTH_CEILING_RPS  ?= 100000
USER_TARGET_RPS   ?= 300
USER_CEILING_RPS  ?= 100000
FACE_TARGET_RPS   ?= 500
FACE_CEILING_RPS  ?= 100000
PERSON_TARGET_RPS ?= 500
PERSON_CEILING_RPS ?= 100000
# photo 子场景独立目标迭代率，覆盖 PHOTO_NOS3_SERVICES 内各脚本
PHOTO_TARGET_RPS  ?= 30
PHOTO_CEILING_RPS ?= 100000
# 各 photo 子场景目标迭代率(默认提高, 可用 <场景名>_TARGET_RPS 覆盖)
PHOTO_COLLECTION_TARGET_RPS        ?= 60
PHOTO_COLLECTION_PHOTO_TARGET_RPS  ?= 40
PHOTO_COMMENT_TARGET_RPS           ?= 50
PHOTO_COMMENT_LIKE_TARGET_RPS      ?= 50
PHOTO_LIKE_TARGET_RPS              ?= 50
# 固定目标压测时长
DURATION ?= 2m
# 容量搜索参数：每档预热后独立测量；不健康即停止，不发送下一档请求。
MAX_ADAPTIVE_WARMUP ?= 10s
MAX_ADAPTIVE_DURATION ?= 30s
MAX_ADAPTIVE_GROWTH_FACTOR ?= 1.5
MAX_ADAPTIVE_MAX_STEPS ?= 8
MAX_ADAPTIVE_P95_MS ?= 500
MAX_ADAPTIVE_ERROR_RATE ?= 0.01
MAX_ADAPTIVE_DROPPED_ITERATIONS ?= 0

RESULTS_DIR  := results
BASELINES    := baselines
CI_DIR       := ci
SERVER_BIN   := ../../target/release/$(BIN_NAME)
SERVER_LOG   := $(CI_DIR)/server.log
SERVER_PID   := $(CI_DIR)/server.pid
COMPOSE      := docker compose -f docker-compose.load.yml

# 多轮输出目录
RUN_PREFIX := run
ifeq ($(strip $(RUN)),)
OUT_DIR := $(RESULTS_DIR)
else
OUT_DIR := $(RESULTS_DIR)/$(RUN_PREFIX)-$(RUN)
endif

.PHONY: all p95-report baseline compare clean help \
        ci-up ci-down run-all run-once photo-nos3 seed collect max-adaptive run-max-adaptive collect-max-adaptive prometheus judge record-baseline report \
        face person

# ── 压测(原有) ──────────────────────────────────────
$(OUT_DIR):
	mkdir -p $@

auth: $(OUT_DIR)       ## 认证模块压测
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e TARGET_RPS=$(AUTH_TARGET_RPS) \
		-e DURATION=$(DURATION) \
		-e SUMMARY_EXPORT=$(OUT_DIR)/auth_summary.json \
		scenarios/auth.js

user: $(OUT_DIR)       ## 用户模块压测
	k6 run \
		-q \
		-e BASE_URL=$(BASE_URL) \
		-e TARGET_RPS=$(USER_TARGET_RPS) \
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
		-e TARGET_RPS=$(FACE_TARGET_RPS) \
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
		-e TARGET_RPS=$(PERSON_TARGET_RPS) \
		-e DURATION=$(DURATION) \
		-e PERSONS=$(PERSONS) \
		-e SUMMARY_EXPORT=$(OUT_DIR)/person_summary.json \
		scenarios/photo/person.js

all: auth user photo face person  ## 运行全部压测(含 S3 场景与 face/person)

# ── 环境编排 ────────────────────────────────────────
ci-up:                 ## 启动压测环境(容器依赖 + release server 进程, taskset 限核)
	$(COMPOSE) up -d --wait
	cd ../.. && cargo build --release --features "$(FEATURES)"
	@echo "Starting server (features: $(FEATURES), cpus: $(SERVER_CPUS))..."
	@cd ../.. && taskset -c 0-$(shell expr $(SERVER_CPUS) - 1) \
		./target/release/$(BIN_NAME) --config $(CI_DIR)/server-config.yml \
		> $(CI_DIR)/server.log 2>&1 & echo $$! > $(SERVER_PID)
	@for i in $$(seq 1 60); do \
		if curl -sf $(BASE_URL)/health > /dev/null; then \
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
run-all:               ## 全场景 target 压测，多轮结果由 collect 归一化
	@set -e; for run in $$(seq 1 $(RUNS)); do \
		echo "===== Run $$run / $(RUNS) ====="; \
		$(MAKE) run-once RUN=$$run; \
		sleep 5; \
	done

max-adaptive:          ## 单轮 SLO 驱动容量搜索
	@run_dir=$(RESULTS_DIR)/max-adaptive-run-$(RUN); \
	for spec in \
		"auth scenarios/auth.js $(AUTH_TARGET_RPS) $(AUTH_CEILING_RPS) 5000" \
		"user scenarios/user.js $(USER_TARGET_RPS) $(USER_CEILING_RPS) 5000" \
		"collection scenarios/photo/collection.js $(PHOTO_COLLECTION_TARGET_RPS) $(PHOTO_CEILING_RPS) 2000" \
		"collection_photo scenarios/photo/collection_photo.js $(PHOTO_COLLECTION_PHOTO_TARGET_RPS) $(PHOTO_CEILING_RPS) 2000" \
		"comment scenarios/photo/comment.js $(PHOTO_COMMENT_TARGET_RPS) $(PHOTO_CEILING_RPS) 2000" \
		"comment_like scenarios/photo/comment_like.js $(PHOTO_COMMENT_LIKE_TARGET_RPS) $(PHOTO_CEILING_RPS) 2000" \
		"photo_like scenarios/photo/photo_like.js $(PHOTO_LIKE_TARGET_RPS) $(PHOTO_CEILING_RPS) 2000" \
		"face scenarios/photo/face.js $(FACE_TARGET_RPS) $(FACE_CEILING_RPS) 2000" \
		"person scenarios/photo/person.js $(PERSON_TARGET_RPS) $(PERSON_CEILING_RPS) 2000"; do \
		set -- $$spec; \
		MAX_ADAPTIVE_WARMUP=$(MAX_ADAPTIVE_WARMUP) MAX_ADAPTIVE_DURATION=$(MAX_ADAPTIVE_DURATION) \
		MAX_ADAPTIVE_GROWTH_FACTOR=$(MAX_ADAPTIVE_GROWTH_FACTOR) MAX_ADAPTIVE_MAX_STEPS=$(MAX_ADAPTIVE_MAX_STEPS) \
		MAX_ADAPTIVE_P95_MS=$(MAX_ADAPTIVE_P95_MS) MAX_ADAPTIVE_ERROR_RATE=$(MAX_ADAPTIVE_ERROR_RATE) \
		MAX_ADAPTIVE_DROPPED_ITERATIONS=$(MAX_ADAPTIVE_DROPPED_ITERATIONS) \
		PHOTOS_PER_USER=$(PHOTOS_PER_USER) PHOTO_COUNT=$(PHOTO_COUNT) FACES=$(FACES) PERSONS=$(PERSONS) \
		$(CI_DIR)/max_adaptive.sh "$$1" "$$2" "$$3" "$$4" "$$5" "$$run_dir/$$1"; \
	done

run-max-adaptive:      ## 多轮 SLO 驱动容量搜索
	@for run in $$(seq 1 $(RUNS)); do \
		echo "===== Capacity run $$run / $(RUNS) ====="; \
		$(MAKE) max-adaptive RUN=$$run; \
	done

collect-max-adaptive:  ## 归一化容量搜索结论 -> results/max-adaptive-normalized/
	$(CI_DIR)/collect_max_adaptive.sh

run-once:              ## 单轮全场景压测(auth + user + photo + face + person)
	$(MAKE) auth RUN=$(RUN)
	$(MAKE) user RUN=$(RUN)
	$(MAKE) photo-nos3 RUN=$(RUN)
	$(MAKE) face RUN=$(RUN)
	$(MAKE) person RUN=$(RUN)

photo-nos3: $(OUT_DIR) ## photo 压测, 仅跑不依赖 S3 的子场景(直接运行独立脚本)
	@set -e; for s in $(PHOTO_NOS3_SERVICES); do \
		case "$$s" in \
			collection)        trps=$(PHOTO_COLLECTION_TARGET_RPS);; \
			collection_photo)  trps=$(PHOTO_COLLECTION_PHOTO_TARGET_RPS);; \
			comment)           trps=$(PHOTO_COMMENT_TARGET_RPS);; \
			comment_like)      trps=$(PHOTO_COMMENT_LIKE_TARGET_RPS);; \
			photo_like)        trps=$(PHOTO_LIKE_TARGET_RPS);; \
			*)                 trps=$(PHOTO_TARGET_RPS);; \
		esac; \
		echo "=== photo scenario: $$s (target=$$trps) ==="; \
		k6 run -q \
			-e BASE_URL=$(BASE_URL) \
			-e TARGET_RPS=$$trps \
			-e DURATION=$(DURATION) \
			-e PHOTOS_PER_USER=$(PHOTOS_PER_USER) \
			-e PHOTO_USERS=$(PHOTO_USERS) \
			-e SUMMARY_EXPORT=$(OUT_DIR)/photo_$${s}_summary.json \
			scenarios/photo/$$s.js; \
	done

seed:                  ## 灌入种子数据(建表 + 压测账号/照片/时间线统计)
	$(CI_DIR)/../seed/seed.sh

# ── 采集层 ──────────────────────────────────────────
collect:               ## 多轮结果归一化(逐指标取中位数) -> results/normalized/
	$(CI_DIR)/collect.sh

prometheus:            ## 抓取服务端 metrics(/metrics)快照 -> results/metrics/
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
