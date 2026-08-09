// tests/load/helpers/common.js
// k6 公共工具函数

import http from "k6/http";
import { Rate, Trend, Counter } from "k6/metrics";

// BASE_URL 必须通过 -e BASE_URL=... 显式传入
const BASE_URL = __ENV.BASE_URL;
if (!BASE_URL) {
    throw new Error(
        "BASE_URL is required. Pass via: k6 run -e BASE_URL=http://host:port script.js",
    );
}

// 数据量配置（与 seed.sql 的 psql 变量对齐）
const AUTH_USERS = parseInt(__ENV.AUTH_USERS || "10000");
const PHOTO_USERS = parseInt(__ENV.PHOTO_USERS || "2000");

/**
 * 生成 auth 测试用户凭据
 * @param {number} vuId - VU ID
 * @returns {{ account: string, password: string }}
 */
export function getTestUserCredentials(vuId) {
    const userId = (vuId % AUTH_USERS) + 1;
    return {
        account: `loadtest_${userId}@test.com`,
        password: "Test123456",
    };
}

/**
 * 生成 photo 测试用户凭据
 * @param {number} vuId - VU ID
 * @returns {{ account: string, password: string }}
 */
export function getPhotoUserCredentials(vuId) {
    const userId = (vuId % PHOTO_USERS) + 1;
    return {
        account: `loadtest_photo_${userId}@test.com`,
        password: "Test123456",
    };
}

/**
 * 创建带 Authorization 头的请求头
 * @param {string} uid - 用户 ID
 * @param {string} token - accessToken
 * @returns {Object} headers
 */
export function authHeaders(uid, token) {
    return {
        "Content-Type": "application/json",
        Authorization: `Bearer ${uid} ${token}`,
    };
}

// ── 共享指标 ──
// k6 自定义指标的 tag 不会自动生成 submetric，必须预定义每个操作的独立指标
const OP_NAMES = [
    // auth
    "login",
    "refresh_token",
    "logout",
    // user
    "get_me",
    "change_nickname",
    "change_password",
    // photo
    "upload_photo",
    "list_photos",
    "timeline_stats",
    // social (collections)
    "create_collection",
    "list_collections",
    "add_photos_to_collection",
    "list_collection_photos",
    "remove_photo_from_collection",
    "delete_collection",
    // social (comments)
    "create_comment",
    "list_comments",
    "delete_comment",
    "like_comment",
    "unlike_comment",
    // face
    "get_faces_by_photo",
    "get_unassigned_face_photos",
    "change_face_belonging",
    // person
    "get_persons",
    "search_persons",
    "get_person_photos",
    "rename_person",
];

const opMetrics = {};
for (const name of OP_NAMES) {
    opMetrics[name] = {
        count: new Counter(`op_${name}_count`),
        errors: new Rate(`op_${name}_errors`),
        duration: new Trend(`op_${name}_duration`, true),
    };
}

/**
 * 记录操作结果到对应的预定义指标
 * @param {string} label - 操作标签（必须在 OP_NAMES 中）
 * @param {{ success: boolean, duration: number }} result
 */
export function recordResult(label, result) {
    const m = opMetrics[label];
    if (!m) return; // 未预见的操作，跳过
    m.count.add(1);
    // k6 Rate: add(true) = pass（成功），add(false) = fail（失败）
    m.errors.add(result.success);
    m.duration.add(result.duration);
}

/**
 * 实时输出单次操作结果
 * 默认仅打印失败(压测日志量巨大); 设 LOG_LEVEL=debug 打印全部
 * @param {string} label - 操作标签
 * @param {{ success: boolean, duration: number, error?: { status: number, body: string } }} result
 */
export function logResult(label, result) {
    const debug = __ENV.LOG_LEVEL === "debug";
    if (result.success && !debug) return;
    const status = result.success ? "\x1b[32mOK\x1b[0m" : "\x1b[31mFAIL\x1b[0m";
    console.log(`  [${status}] ${label} ${Math.round(result.duration)}ms`);
    if (!result.success && result.error) {
        const body = String(result.error.body ?? "");
        const shown = body.length > 200 ? body.substring(0, 200) + "..." : body;
        console.error(`    → HTTP ${result.error.status ?? "?"}: ${shown}`);
    }
}

/**
 * 构建 arrival-rate 负载模型 options(双模式)
 *
 * LOAD_MODE=target(默认): constant-arrival-rate 固定迭代率, 稳定可对比
 * LOAD_MODE=max:          ramping-arrival-rate 逐步加压找系统上限
 *
 * 迭代率可用 __ENV.TARGET_RPS / __ENV.MAX_RPS 覆盖(单位: 迭代/秒)。
 * 单 op 迭代脚本中迭代率即请求 QPS; 完整流程迭代脚本中 QPS ≈ 迭代率 × 每迭代请求数。
 */
export function buildLoadOptions({
    targetRps,
    maxRps,
    preAllocatedVUs = 100,
    maxVUs = 200,
}) {
    const mode = __ENV.LOAD_MODE || "target";
    const target = parseInt(__ENV.TARGET_RPS || String(targetRps), 10);
    const max = parseInt(__ENV.MAX_RPS || String(maxRps), 10);
    const pre = parseInt(__ENV.PRE_ALLOCATED_VUS || String(preAllocatedVUs), 10);
    const mv = parseInt(__ENV.MAX_VUS || String(maxVUs), 10);
    const duration = __ENV.DURATION || "2m";

    if (mode === "max") {
        const ramp = Math.max(1, Math.floor(max * 0.1));
        return {
            scenarios: {
                load: {
                    executor: "ramping-arrival-rate",
                    startRate: ramp,
                    timeUnit: "1s",
                    preAllocatedVUs: pre,
                    maxVUs: mv,
                    stages: [
                        { duration: "1m", target: ramp },
                        { duration: "2m", target: max },
                        { duration: "1m", target: max },
                    ],
                },
            },
        };
    }
    return {
        scenarios: {
            load: {
                executor: "constant-arrival-rate",
                rate: target,
                timeUnit: "1s",
                duration,
                preAllocatedVUs: pre,
                maxVUs: mv,
            },
        },
    };
}

/**
 * 自定义 summary 输出（替换 k6 默认的 metrics dump）
 * 在 scenario 中: export { handleSummary } from "../helpers/common.js";
 * @param {Object} data - k6 metrics data
 * @returns {Object} { stdout: string, [exportPath]: string }
 */
export function printSummary(data) {
    const lines = [];
    lines.push("");
    lines.push("┌─────────────────────────────────────────────────────────────┐");
    lines.push("│                      Load Test Summary                      │");
    lines.push("├──────────────────────────┬──────────┬──────────┬────────────┤");
    lines.push("│ Operation                │  Reqs    │  Fails   │  P95 (ms)  │");
    lines.push("├──────────────────────────┼──────────┼──────────┼────────────┤");

    // Use http_req_duration for overall line
    const httpDur = data.metrics.http_req_duration?.values || {};
    const httpReqs = data.metrics.http_reqs?.values || {};
    const httpFails = data.metrics.http_req_failed?.values || {};

    const p95 = httpDur["p(95)"] ?? 0;
    const totalReqs = httpReqs["count"] ?? 0;
    const failRate = httpFails["rate"] ?? 0;
    const failCount = Math.round(totalReqs * failRate);

    lines.push(
        `│ ${"ALL".padEnd(24)} │ ${String(totalReqs).padStart(8)} │ ${String(failCount).padStart(8)} │ ${p95.toFixed(1).padStart(10)} │`,
    );

    lines.push("├──────────────────────────┼──────────┼──────────┼────────────┤");

    // Extract per-operation metrics (predefined per-op metrics: op_<name>_count/errors/duration)
    for (const op of OP_NAMES) {
        const countMetric = data.metrics[`op_${op}_count`];
        if (!countMetric) continue; // this op was not used in the test
        const count = countMetric.values?.count ?? 0;
        const errors = data.metrics[`op_${op}_errors`]?.values?.fails ?? 0;
        const p95Val = data.metrics[`op_${op}_duration`]?.values?.["p(95)"] ?? 0;
        lines.push(
            `│ ${op.padEnd(24)} │ ${String(Math.round(count)).padStart(8)} │ ${String(Math.round(errors)).padStart(8)} │ ${p95Val.toFixed(1).padStart(10)} │`,
        );
    }

    lines.push("└──────────────────────────┴──────────┴──────────┴────────────┘");
    lines.push("");

    const summary = lines.join("\n");

    // Also write JSON export if path specified
    const result = { stdout: summary };
    if (__ENV.SUMMARY_EXPORT) {
        result[__ENV.SUMMARY_EXPORT] = JSON.stringify(data, null, 2);
    }
    return result;
}

export { BASE_URL };
