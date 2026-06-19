// tests/load/scenarios/auth.js
// 认证模块独立压测场景

import { sleep } from "k6";
import { getTestUserCredentials, printSummary } from "../helpers/common.js";
import { initSession, refreshSession, logout } from "../helpers/session.js";

export { printSummary as handleSummary };

export const options = {
    stages: [
        { duration: "30s", target: 100 },
        { duration: "2m", target: 100 },
        { duration: "30s", target: 200 },
        { duration: "2m", target: 200 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<200"],
        http_req_failed: ["rate<0.01"],
    },
};

export default function () {
    const { account, password } = getTestUserCredentials(__VU);

    // 1. 登录
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.5);

    // 2. 循环刷新 token（模拟用户长期在线）
    for (let i = 0; i < 10; i++) {
        refreshSession();
        sleep(0.5);
    }

    // 3. 登出
    logout();

    sleep(0.5);
}
