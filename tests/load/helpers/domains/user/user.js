// tests/load/helpers/domains/user/user.js
// 用户模块操作函数

import http from "k6/http";
import { BASE_URL } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

/**
 * 获取当前用户信息
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function getMe() {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/user/me`, { headers });
    return {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
    };
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
    return {
        success: res.status === 200,
        duration: res.timings.duration,
    };
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
    return {
        success: res.status === 200,
        duration: res.timings.duration,
    };
}
