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

export { BASE_URL };
