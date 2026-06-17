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
const PHOTO_USERS = parseInt(__ENV.PHOTO_USERS || "20");

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
export const opDuration = new Trend("op_duration", true);
export const opErrors = new Rate("op_errors");
export const opCount = new Counter("op_count");

/**
 * 记录操作结果到共享指标
 * @param {string} label - 操作标签
 * @param {{ success: boolean, duration: number }} result
 */
export function recordResult(label, result) {
    opDuration.add(result.duration, { operation: label });
    opErrors.add(!result.success, { operation: label });
    opCount.add(1, { operation: label });
}

/**
 * 实时输出单次操作结果
 * @param {string} label - 操作标签
 * @param {{ success: boolean, duration: number }} result
 */
export function logResult(label, result) {
    const status = result.success ? "\x1b[32mOK\x1b[0m" : "\x1b[31mFAIL\x1b[0m";
    console.log(`  [${status}] ${label} ${Math.round(result.duration)}ms`);
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

    const opValues = data.metrics.op_duration?.values || {};
    const opTags = opValues["avg"] !== undefined ? null : null; // just check existence

    // Collect per-operation stats from op_duration tagged values
    const opMetrics = {};
    if (data.metrics.op_duration?.values) {
        for (const [key, val] of Object.entries(data.metrics.op_duration.values)) {
            // tagged metrics have format like "op_duration{operation:label}"
            const match = key.match(/\{operation:(.+?)\}/);
            if (match) {
                const label = match[1];
                if (!opMetrics[label]) opMetrics[label] = {};
                opMetrics[label].avg = val;
            }
        }
    }

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

    // Per-operation from custom metrics
    const opErrors = data.metrics.op_errors?.values || {};
    const opCounts = data.metrics.op_count?.values || {};

    // Try to extract tagged values
    const countByOp = extractTagged(opCounts);
    const errorByOp = extractTagged(opErrors);
    const durationByOp = extractTagged(data.metrics.op_duration?.values || {});

    const allOps = new Set([...Object.keys(countByOp), ...Object.keys(errorByOp), ...Object.keys(durationByOp)]);
    for (const op of [...allOps].sort()) {
        const count = countByOp[op] ?? 0;
        const errors = errorByOp[op] ?? 0;
        const p95Val = durationByOp[op] ?? 0;
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

/**
 * Extract tagged metric values into { label: value } map
 */
function extractTagged(values) {
    const result = {};
    for (const [key, val] of Object.entries(values)) {
        const match = key.match(/\{operation:(.+?)\}/);
        if (match) {
            result[match[1]] = val;
        } else if (!key.includes("{")) {
            // untagged — skip or use as fallback
        }
    }
    return result;
}

export { BASE_URL };
