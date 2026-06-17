// tests/load/scenarios/user.js
// 用户模块独立压测场景

import { sleep } from "k6";
import { Rate, Trend } from "k6/metrics";
import { getTestUserCredentials, recordResult } from "../helpers/common.js";
import { initSession, logout } from "../helpers/session.js";
import { getMe, changeNickname, changePassword } from "../helpers/domains/user/user.js";

const userErrorRate = new Rate("user_errors");
const userDuration = new Trend("user_duration");

export const options = {
    stages: [
        { duration: "30s", target: 20 },
        { duration: "1m", target: 20 },
        { duration: "30s", target: 50 },
        { duration: "1m", target: 50 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<200"],
        http_req_failed: ["rate<0.01"],
        user_errors: ["rate<0.01"],
    },
};

export default function () {
    const { account, password } = getTestUserCredentials(__VU);

    // 登录
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 1. 获取个人信息
    let result = getMe();
    recordResult("get_me", result);
    userErrorRate.add(!result.success);
    userDuration.add(result.duration);
    if (!result.success) return;

    sleep(0.5);

    // 2. 修改昵称
    result = changeNickname(`Updated ${__VU} ${Date.now()}`);
    recordResult("change_nickname", result);
    userErrorRate.add(!result.success);
    userDuration.add(result.duration);

    sleep(0.5);

    // 3. 再次获取个人信息（验证修改生效）
    result = getMe();
    recordResult("get_me", result);
    userErrorRate.add(!result.success);
    userDuration.add(result.duration);

    sleep(0.5);

    // 4. 修改密码（使用相同密码，避免影响后续测试）
    result = changePassword(password, password);
    recordResult("change_password", result);
    userErrorRate.add(!result.success);
    userDuration.add(result.duration);

    sleep(0.5);

    // 5. 登出
    logout();

    sleep(0.5);
}
