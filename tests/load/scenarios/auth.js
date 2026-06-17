// tests/load/scenarios/auth.js
// 认证模块独立压测场景

import { sleep } from "k6";
import { Rate, Trend } from "k6/metrics";
import { getTestUserCredentials, printSummary } from "../helpers/common.js";
import { initSession, refreshSession, logout } from "../helpers/session.js";

export { printSummary as handleSummary };

// 自定义指标
const loginErrorRate = new Rate("login_errors");
const loginDuration = new Trend("login_duration");
const tokenErrorRate = new Rate("token_refresh_errors");
const tokenDuration = new Trend("token_refresh_duration");

export const options = {
    stages: [
        { duration: "30s", target: 50 },
        { duration: "2m", target: 50 },
        { duration: "30s", target: 100 },
        { duration: "2m", target: 100 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<200"],
        http_req_failed: ["rate<0.01"],
        login_errors: ["rate<0.01"],
        token_refresh_errors: ["rate<0.01"],
    },
};

export default function () {
    const { account, password } = getTestUserCredentials(__VU);

    // 1. 登录
    const loginStart = Date.now();
    const session = initSession(account, password);
    const loginDur = Date.now() - loginStart;

    loginDuration.add(loginDur);
    loginErrorRate.add(!session);

    if (!session) return;

    sleep(0.5);

    // 2. 循环刷新 token（模拟用户长期在线）
    for (let i = 0; i < 10; i++) {
        const tokenStart = Date.now();
        const ok = refreshSession();
        const tokenDur = Date.now() - tokenStart;

        tokenDuration.add(tokenDur);
        tokenErrorRate.add(!ok);

        sleep(0.5);
    }

    // 3. 登出
    logout();

    sleep(0.5);
}
