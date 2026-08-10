# tests/load/Makefile
# Load test entry point — run k6 scenarios against a running server
#
# 压测体系分层:
#   编排层   : ci-up / ci-down / run-all / run-once / seed
#   采集层   : collect / prometheus
#   判定层   : judge / record-baseline
#   报告层   : report
#   原有层   : auth / user / photo / p95-report / baseline / compare

REMOTE_HOST ?= 127.0.0.1
SERVER_PORT ?= 7985

# 构建与运行
FEATURES ?= metrics,auth,user,photo,face-engine
BIN_NAME  ?= memory-seek-server
RUNS      ?= 3          # 每场景轮次, collect 取中位数
INCLUDE_S3_PATHS ?= false

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
AUTH_TARGET_RPS   ?= 400
AUTH_MAX_RPS      ?= 100000
USER_TARGET_RPS   ?= 300
USER_MAX_RPS      ?= 100000
FACE_TARGET_RPS   ?= 500
FACE_MAX_RPS      ?= 100000
PERSON_TARGET_RPS ?= 500
PERSON_MAX_RPS    ?= 100000
PHOTO_TARGET_RPS  ?= 30
PHOTO_MAX_RPS     ?= 100000
# 压测时长(target 模式, 如 2m/3m; max 模式 stages 固定 1m+2m+1m)
DURATION ?= 2m

# ── 多 server 实例并行 ──────────────────────────────
# 单机多 server 进程并行压测: 每实例绑定独立核心 + 独立端口 + 独立模块(组内串行),
# 实例间通过 make -j 并行, 总时长 = max(各实例), 且各实例内部串行保证 QPS 准确。
#
# 实例表: <name>:<port>:<k6脚本列表>:<summary名列表>:<target_rps>:<max_rps>
#   name         实例标识(pid/日志文件前缀)
#   port         独立监听端口
#   k6脚本列表   scenarios/ 下的脚本路径, 逗号分隔(实例内串行执行)
#   summary名列表 summary 输出文件名, 逗号分隔(与脚本一一对应)
#   target_rps   target 模式目标迭代率(单 op 场景即 QPS)
#   max_rps      max 模式软上限
# 核心区间不在此表: 由 SERVER_CPUS(每实例核数)按实例序号自动分配, 便于 CI 控制
SERVER_INSTANCES ?= \
	server-a:7985:auth.js,user.js:auth,user:400:100000 \
	server-b:7987:photo/collection.js,photo/collection_photo.js:photo_collection,photo_collection_photo:30:100000 \
	server-c:7989:photo/comment.js,photo/comment_like.js:photo_comment,photo_comment_like:30:100000 \
	server-d:7991:photo/face.js,photo/person.js:face,person:500:100000

# 每实例占用核数(taskset 绑定, 控制变量: 压测对比不随宿主核数漂移)
SERVER_CPUS ?= 2
# 实例核心起始偏移(默认 0: 实例 1 绑 0-(n-1), 实例 2 绑 n-(2n-1), ...)
CPU_BASE ?= 0
# 宿主可用核数(自动检测; CI runner 可覆盖): 核心分配不超过该上限
TOTAL_CPUS ?= $(shell nproc 2>/dev/null || echo 4)

# 每实例 DB/Redis 连接池(共享同一 PG/Redis, 连接池均分避免超限)
# PG 容器已调 max_connections=256; Redis 无严格上限
DB_MAX_CONNECTIONS  ?= 32
REDIS_MAX_CONNECTIONS ?= 16

# 并行实例数(组间并行度): 默认按核数自适应 floor(TOTAL_CPUS / SERVER_CPUS), 至少 1
PARALLEL ?= $(shell expr $(TOTAL_CPUS) / $(SERVER_CPUS))

# 解析实例表辅助: $(call inst_<字段>,<实例>)
# 字段: 1=name 2=port 3=script列表 4=summary列表 5=target_rps 6=max_rps
inst_name  = $(word 1,$(subst :, ,$(1)))
inst_port  = $(word 2,$(subst :, ,$(1)))
inst_scripts= $(subst $(comma), ,$(word 3,$(subst :, ,$(1))))
inst_sums   = $(subst $(comma), ,$(word 4,$(subst :, ,$(1))))
inst_trps  = $(word 5,$(subst :, ,$(1)))
inst_mrps  = $(word 6,$(subst :, ,$(1)))
# script:summary 对列表(与脚本/摘要一一对应), 供 run-inst 串行遍历
inst_pairs = $(join $(addsuffix :,$(call inst_scripts,$(1))),$(call inst_sums,$(1)))

# 实例序号(1-based)与核心区间
# 第 N 个实例绑核心 [CPU_BASE + (N-1)*SERVER_CPUS, CPU_BASE + N*SERVER_CPUS - 1],
# 核心编号对 TOTAL_CPUS 取模, 超出可用核时回绕(适合实例数×核数 > 总核数场景)
INST_NAMES := $(foreach inst,$(SERVER_INSTANCES),$(call inst_name,$(inst)))
define COMPUTE_INST_CPU
$(eval INST_CPU_$(call inst_name,$(1)) := $(shell n=$$(echo "$(INST_NAMES)" | tr ' ' '\n' | grep -n '^$(call inst_name,$(1))$$' | cut -d: -f1); \
	lo=$$(( ( $(CPU_BASE) + (n-1) * $(SERVER_CPUS) ) % $(TOTAL_CPUS) )); \
	hi=$$(( ( $(CPU_BASE) + n * $(SERVER_CPUS) - 1 ) % $(TOTAL_CPUS) )); \
	if [ $$lo -le $$hi ]; then echo "$$lo-$$hi"; else echo "$$lo-$$(( $(TOTAL_CPUS) - 1 )) $$(( 0 ))-$$hi"; fi))
endef
$(foreach inst,$(SERVER_INSTANCES),$(eval $(call COMPUTE_INST_CPU,$(inst))))

inst_cpu = $(INST_CPU_$(1))

comma := ,

RESULTS_DIR  := results
BASELINES    := baselines
CI_DIR       := ci
SERVER_BIN   := ../../target/release/$(BIN_NAME)
COMPOSE      := docker compose -f docker-compose.load.yml
BASE_URL     := http://$(REMOTE_HOST):$(SERVER_PORT)

# max 模式结果目录(与 ci/env.sh 对齐, collect-max 依赖)
MAX_RUNS_DIR      := $(RESULTS_DIR)/max-run
MAX_NORMALIZED_DIR := $(RESULTS_DIR)/max-normalized

# 多轮输出目录: 指定 RUN 时写入 run-<n>(target) 或 max-run-<n>(max), 否则沿用原路径
RUN_PREFIX := $(if $(filter max,$(LOAD_MODE)),max-run,run)
ifeq ($(strip $(RUN)),)
OUT_DIR := $(RESULTS_DIR)
else
OUT_DIR := $(RESULTS_DIR)/$(RUN_PREFIX)-$(RUN)
endif

.PHONY: all p95-report baseline compare clean help \
        ci-up ci-down run-all run-max run-once seed collect collect-max prometheus judge record-baseline report \
        face person \
        $(foreach inst,$(SERVER_INSTANCES),ci-up-$(call inst_name,$(inst)) ci-down-$(call inst_name,$(inst)) run-inst-$(call inst_name,$(inst)))

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
# 为每个实例生成独立的启停 target: ci-up-<name> / ci-down-<name> / run-inst-<name>
define INSTANCE_CI_RULES
ci-up-$(call inst_name,$(1)):
	@echo "Starting instance $(call inst_name,$(1)) (port $(call inst_port,$(1)), cpus $(call inst_cpu,$(call inst_name,$(1))))..."
	@taskset -c $(call inst_cpu,$(call inst_name,$(1))) \
		env MEMORY_SEEK_SERVER__PORT=$(call inst_port,$(1)) \
			MEMORY_SEEK_DATABASE__MAX_CONNECTIONS=$(DB_MAX_CONNECTIONS) \
			MEMORY_SEEK_REDIS__MAX_CONNECTIONS=$(REDIS_MAX_CONNECTIONS) \
			$(SERVER_BIN) --config $(CI_DIR)/server-config.yml \
			> $(CI_DIR)/$(call inst_name,$(1)).log 2>&1 & \
			echo $$! > $(CI_DIR)/$(call inst_name,$(1)).pid
	@for i in $$(seq 1 60); do \
		if curl -sf http://127.0.0.1:$(call inst_port,$(1))/health > /dev/null; then \
			echo "Instance $(call inst_name,$(1)) ready at http://127.0.0.1:$(call inst_port,$(1))"; \
			exit 0; \
		fi; \
		sleep 1; \
	done; \
	echo "ERROR: instance $(call inst_name,$(1)) failed to start"; \
	tail -50 $(CI_DIR)/$(call inst_name,$(1)).log; exit 1
ci-down-$(call inst_name,$(1)):
	@if [ -f $(CI_DIR)/$(call inst_name,$(1)).pid ]; then \
		kill $$(cat $(CI_DIR)/$(call inst_name,$(1)).pid) 2>/dev/null || true; \
		rm -f $(CI_DIR)/$(call inst_name,$(1)).pid; \
	fi
run-inst-$(call inst_name,$(1)): $(OUT_DIR)
	@echo "=== instance $(call inst_name,$(1)): $(call inst_scripts,$(1)) (port $(call inst_port,$(1))) ==="; \
	scripts='$(call inst_scripts,$(1))'; sums='$(call inst_sums,$(1))'; \
	i=1; \
	for s in $$$$scripts; do \
		sum=$$$$(echo $$$$sums | tr ' ' '\n' | sed -n "$$$${i}p"); \
		echo "  --- module: $$$$s (summary: $$$$sum) ---"; \
		k6 run -q \
			-e BASE_URL=http://127.0.0.1:$(call inst_port,$(1)) \
			-e LOAD_MODE=$(LOAD_MODE) \
			-e TARGET_RPS=$(call inst_trps,$(1)) \
			-e MAX_RPS=$(call inst_mrps,$(1)) \
			-e DURATION=$(DURATION) \
			-e PHOTO_COUNT=$(PHOTO_COUNT) \
			-e FACES=$(FACES) \
			-e PERSONS=$(PERSONS) \
			-e SUMMARY_EXPORT=$(OUT_DIR)/$$$${sum}_summary.json \
			scenarios/$$$$s || echo "[warn] module $$$$s 失败"; \
		i=$$$$((i+1)); \
	done || echo "[warn] instance $(call inst_name,$(1)) 失败"
endef
$(foreach inst,$(SERVER_INSTANCES),$(eval $(call INSTANCE_CI_RULES,$(inst))))

ci-up:                 ## 启动压测环境(容器 + 全部 server 实例, 各自绑核)
	$(COMPOSE) up -d --wait
	@if [ "$(SKIP_BUILD)" != "1" ]; then cd ../.. && cargo build --release --features "$(FEATURES)"; fi
	@$(foreach inst,$(SERVER_INSTANCES),$(MAKE) -s ci-up-$(call inst_name,$(inst));)

ci-down:               ## 停止压测环境(全部 server 实例 + 容器)
	@$(foreach inst,$(SERVER_INSTANCES),$(MAKE) -s ci-down-$(call inst_name,$(inst));)
	$(COMPOSE) down

# ── 压测(编排层) ────────────────────────────────────
run-all:               ## 全场景压测(LOAD_MODE=target|max), 多轮取中位数
	@for run in $$(seq 1 $(RUNS)); do \
		echo "===== Run $$run / $(RUNS) ====="; \
		$(MAKE) run-once RUN=$$run; \
		sleep 5; \
	done
	@$(MAKE) $(if $(filter max,$(LOAD_MODE)),collect-max,collect)

# max 极限压测: 与 run-all 同, 但结果输出到 max-run-<n> / max-normalized
run-max:               ## max 极限压测(LOAD_MODE=max), 输出 max-run-* 并归一化
	@$(MAKE) run-all LOAD_MODE=max RUNS=$(RUNS)
	@$(MAKE) collect-max

collect-max:           ## 归一化 max 结果 -> max-normalized/
	COLLECT_RUNS_DIR=$(MAX_RUNS_DIR) COLLECT_OUT_DIR=$(MAX_NORMALIZED_DIR) $(CI_DIR)/collect.sh

run-once:              ## 单轮全场景压测(各 server 实例并行, 实例内单模块串行)
	+@$(MAKE) -j$(PARALLEL) \
		RUN=$(RUN) \
		$(foreach inst,$(SERVER_INSTANCES),run-inst-$(call inst_name,$(inst))) \
		|| echo "[warn] 部分实例压测失败(见 results)"

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
