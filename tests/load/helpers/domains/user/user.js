// tests/load/helpers/domains/user/user.js
// 用户模块操作函数

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

/**
 * 获取当前用户信息
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function getMe() {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/user/me`, { headers });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("get_me", result);
    return result;
}

/**
 * 修改昵称
 * @param {string} nickname - 新昵称
 * @returns {{ success: boolean, duration: number }}
 */
export function changeNickname(nickname) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.patch(
        `${BASE_URL}/user/nickname`,
        JSON.stringify({ newNickname: nickname }),
        { headers },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("change_nickname", result);
    return result;
}

/**
 * 修改密码
 * @param {string} oldPassword - 旧密码
 * @param {string} newPassword - 新密码
 * @returns {{ success: boolean, duration: number }}
 */
export function changePassword(oldPassword, newPassword) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.patch(
        `${BASE_URL}/user/password`,
        JSON.stringify({ oldPassword, newPassword }),
        { headers },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("change_password", result);
    return result;
}

/**
 * 生成邀请码
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function generateInviterCode() {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(`${BASE_URL}/user/inviter-code`, null, { headers });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("generate_inviter_code", result);
    return result;
}

/**
 * 批量获取用户信息
 * @param {string[]} userIds - 用户 ID 数组
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function getUserInfoBatch(userIds) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(
        `${BASE_URL}/user/batch`,
        JSON.stringify({ userIds }),
        { headers },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("get_user_info_batch", result);
    return result;
}
