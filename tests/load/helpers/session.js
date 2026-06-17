// tests/load/helpers/session.js
// 会话管理：登录、token 刷新、登出、模块级状态

import http from "k6/http";
import { BASE_URL, authHeaders, logResult } from "./common.js";

let _session = null; // { uid, token, refreshToken }
let _opCount = 0; // 操作计数，用于 token 刷新策略

/**
 * 登录并初始化会话
 * @param {string} account - 邮箱账号
 * @param {string} password - 密码
 * @returns {{ uid: number, token: string, refreshToken: string }|null}
 */
export function initSession(account, password) {
    const res = http.post(
        `${BASE_URL}/auth/login`,
        JSON.stringify({ account, password }),
        { headers: { "Content-Type": "application/json" } },
    );
    const ok = res.status === 200;
    if (!ok) {
        console.error(`Login failed for ${account}: ${res.status} ${res.body}`);
    }
    logResult("login", { success: ok, duration: res.timings.duration });
    if (!ok) return null;
    _session = {
        uid: res.json("data.id"),
        token: res.json("data.accessToken"),
        refreshToken: res.json("data.refreshToken"),
    };
    _opCount = 0;
    return _session;
}

/**
 * 获取当前会话
 * @returns {{ uid: number, token: string, refreshToken: string }|null}
 */
export function getSession() {
    return _session;
}

/**
 * 获取当前会话用户 ID
 * @returns {number|null}
 */
export function getSessionUid() {
    return _session?.uid ?? null;
}

/**
 * 获取带认证头的请求头
 * @returns {Object|null}
 */
export function getSessionHeaders() {
    if (!_session) return null;
    return authHeaders(_session.uid, _session.token);
}

/**
 * 刷新 token
 * @returns {boolean} 是否成功
 */
export function refreshSession() {
    if (!_session?.refreshToken) return false;
    const res = http.post(`${BASE_URL}/auth/token`, null, {
        headers: {
            "Content-Type": "application/json",
            "x-user-id": String(_session.uid),
            "x-refresh-token": _session.refreshToken,
        },
    });
    const ok = res.status === 200;
    logResult("refresh_token", { success: ok, duration: res.timings.duration });
    if (ok) {
        _session.token = res.json("data.accessToken");
        _session.refreshToken = res.json("data.refreshToken");
        _opCount = 0;
        return true;
    }
    console.error(`Token refresh failed: ${res.status} ${res.body}`);
    return false;
}

/**
 * 检查并自动刷新 token（每 25 个操作刷新一次）
 */
export function maybeRefreshSession() {
    _opCount++;
    if (_opCount >= 25) {
        refreshSession();
    }
}

/**
 * 登出并清除会话
 */
export function logout() {
    if (!_session) return;
    const res = http.post(`${BASE_URL}/user/logout`, null, {
        headers: getSessionHeaders(),
    });
    logResult("logout", { success: res.status === 200, duration: res.timings.duration });
    _session = null;
}
