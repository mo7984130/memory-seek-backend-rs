// tests/load/scripts/auth/auth_service.js
// 认证模块压测

import http from "k6/http";
import { check, sleep } from "k6";
import { Rate, Trend } from "k6/metrics";
import {
    login,
    getTestUserCredentials,
    authHeaders,
    BASE_URL,
} from "../../helpers/common.js";

// 自定义指标
const loginErrorRate = new Rate("login_errors");
const loginDuration = new Trend("login_duration");
const tokenErrorRate = new Rate("token_refresh_errors");
const tokenDuration = new Trend("token_refresh_duration");

export const options = {
    stages: [
        { duration: "30s", target: 50 },
        { duration: "1m", target: 50 },
        { duration: "30s", target: 100 },
        { duration: "1m", target: 100 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<200"],
        http_req_failed: ["rate<0.01"],
        login_errors: ["rate<0.01"],
        token_refresh_errors: ["rate<0.01"],
    },
};

export function setup() {
    return {};
}

export default function () {
    const { account, password } = getTestUserCredentials(__VU);

    // 1. 登录
    const loginRes = http.post(
        `${BASE_URL}/auth/login`,
        JSON.stringify({
            account,
            password,
        }),
        {
            headers: { "Content-Type": "application/json" },
        },
    );

    check(loginRes, {
        "login status is 200": (r) => r.status === 200,
        "login has token": (r) => r.json("data.accessToken") !== undefined,
    });

    loginErrorRate.add(loginRes.status !== 200);
    loginDuration.add(loginRes.timings.duration);

    if (loginRes.status !== 200) {
        console.error(`Login failed: ${loginRes.body}`);
        return;
    }

    const uid = loginRes.json("data.id");
    const token = loginRes.json("data.accessToken");
    const refreshToken = loginRes.json("data.refreshToken");

    sleep(0.5);

    // 2. Token 刷新
    if (refreshToken) {
        const tokenRes = http.post(`${BASE_URL}/auth/token`, null, {
            headers: {
                "Content-Type": "application/json",
                "x-user-id": uid.toString(),
                "x-refresh-token": refreshToken,
            },
        });

        check(tokenRes, {
            "token refresh status is 200": (r) => r.status === 200,
        });

        tokenErrorRate.add(tokenRes.status !== 200);
        tokenDuration.add(tokenRes.timings.duration);
    }

    sleep(0.5);

    // 3. 访问受保护接口验证 token 有效性
    const meRes = http.get(`${BASE_URL}/user/me`, {
        headers: authHeaders(uid, token),
    });

    check(meRes, {
        "me status is 200": (r) => r.status === 200,
    });

    sleep(1);
}
