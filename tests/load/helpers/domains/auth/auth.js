// tests/load/helpers/domains/auth/auth.js
// 认证模块操作函数（注册、发送验证码等辅助操作）
// 注意：主要的登录/刷新/登出逻辑在 session.js 中

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";

/**
 * 注册新用户
 * @param {string} username - 用户名
 * @param {string} email - 邮箱
 * @param {string} password - 密码
 * @param {string} [verificationCode] - 验证码（可选）
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function register(username, email, password, verificationCode = "") {
    const res = http.post(
        `${BASE_URL}/auth/register`,
        JSON.stringify({ username, email, password, verificationCode }),
        { headers: { "Content-Type": "application/json" } },
    );
    const result = {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
    };
    logResult("register", result);
    return result;
}

/**
 * 发送邮箱验证码
 * @param {string} email - 邮箱
 * @returns {{ success: boolean, duration: number }}
 */
export function sendEmailCode(email) {
    const res = http.post(
        `${BASE_URL}/auth/verification-codes`,
        JSON.stringify({ email }),
        { headers: { "Content-Type": "application/json" } },
    );
    const result = {
        success: res.status === 200,
        duration: res.timings.duration,
    };
    logResult("send_email_code", result);
    return result;
}
