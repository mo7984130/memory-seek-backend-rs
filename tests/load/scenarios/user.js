// tests/load/scenarios/user.js
// 用户模块独立压测场景

import { sleep } from "k6";
import { getTestUserCredentials, recordResult, printSummary } from "../helpers/common.js";
import { initSession, logout } from "../helpers/session.js";
import { getMe, changeNickname, changePassword } from "../helpers/domains/user/user.js";

export { printSummary as handleSummary };

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
    if (!result.success) return;

    sleep(0.5);

    // 2. 修改昵称（限 20 字符）
    result = changeNickname(`U${__VU}_${String(Date.now()).slice(-6)}`);
    recordResult("change_nickname", result);

    sleep(0.5);

    // 3. 再次获取个人信息（验证修改生效）
    result = getMe();
    recordResult("get_me", result);

    sleep(0.5);

    // 4. 修改密码（先改临时密码，再改回原密码，保证后续测试不受影响）
    //    注意：服务端 change_password 会调用 logout 清除 token，每次改完需重新登录
    const tempPassword = "Temp@12345";
    result = changePassword(password, tempPassword);
    recordResult("change_password", result);

    sleep(0.3);

    // 用临时密码重新登录（因服务端已清除 token）
    if (!initSession(account, tempPassword)) return;

    sleep(0.3);

    result = changePassword(tempPassword, password);
    recordResult("change_password", result);

    sleep(0.3);

    // 用原密码重新登录（恢复 session 以便后续 logout）
    if (!initSession(account, password)) return;

    sleep(0.5);

    // 5. 登出
    logout();

    sleep(0.5);
}
