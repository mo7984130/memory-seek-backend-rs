// tests/load/scenarios/user.js
// 用户模块压测场景 — arrival-rate 负载模型, setup 预登录
//
// 双模式(target/max), 会话由 setup 预登录(login 不计入压测窗口),
// 迭代 = 单个业务请求(get_me / change_nickname), 会话全程复用。
// 说明: change_password 会使服务端登出并改变账号密码, 不适合高频压测, 故不包含。

import {
    getTestUserCredentials,
    setupPreLogin,
    sessionFromData,
    recordResult,
    printSummary,
    buildLoadOptions,
} from "../helpers/common.js";
import {
    setSession,
    initSession,
    maybeRefreshSession,
} from "../helpers/session.js";
import { getMe, changeNickname } from "../helpers/domains/user/user.js";

export { printSummary as handleSummary };

const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);

export const options = buildLoadOptions({
    targetRps: 300,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 5000,
});

// setup 预登录: login 不计入压测窗口
export function setup() {
    return setupPreLogin(getTestUserCredentials, PRE_ALLOCATED_VUS);
}

export default function (data) {
    const session = sessionFromData(data, __VU);
    if (session) {
        setSession(session);
    } else {
        // 兜底: 预登录缺失(VU 超出预分配或登录失败), 现场登录
        const { account, password } = getTestUserCredentials(__VU);
        initSession(account, password);
        return;
    }

    maybeRefreshSession();

    if (Math.random() < 0.6) {
        recordResult("get_me", getMe());
    } else {
        recordResult("change_nickname", changeNickname(`U${__VU}_${String(Date.now()).slice(-6)}`));
    }
}
