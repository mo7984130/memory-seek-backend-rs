import { check, sleep } from 'k6';
import { config } from '../common/config.js';
import { login } from '../common/auth.js';
import {
    getUserByVUAndScenario,
    randomSleep,
} from '../common/skill.js';
import {
    getUserInfo,
    changeNickname,
    generateInviterCode,
    changePassword,
    logout,
} from './user_service.js';
import { generateRandomPassword } from '../utils/password.js';

/**
 * ==================== User 模块压测场景 ====================
 * 
 * 认证安全规则（详见 skill.js）：
 * 1. 登出后，所有 token 失效
 * 2. 重新登录后，之前的 token 失效
 * 3. 修改密码后，当前 token 失效（服务器内部会调用登出，清除所有 token）
 * 
 * 场景设计原则：
 * - 每个场景开始时重新登录获取最新 token
 * - 避免在多个操作之间共享 token
 * - 【重要】所有场景都使用 getUserByVU() 分配固定用户，避免并发 token 竞争
 * 
 * 为什么必须用 getUserByVU()：
 * - 重新登录会使旧 token 失效
 * - 如果多个 VU 登录同一用户，后面的登录会使前面的 token 失效
 * - 这会导致前面的 VU 操作失败（token 无效）
 */

const scenarioConfigs = {
    get_user_info: {
        executor: 'constant-vus',
        vus: 20,
        duration: '5m',
        exec: 'getUserInfoScenario',
        tags: { scenario: 'get_user_info' },
    },
    change_nickname: {
        executor: 'constant-vus',
        vus: 15,
        duration: '5m',
        exec: 'changeNicknameScenario',
        tags: { scenario: 'change_nickname' },
    },
    generate_inviter_code: {
        executor: 'constant-vus',
        vus: 10,
        duration: '5m',
        exec: 'generateInviterCodeScenario',
        tags: { scenario: 'generate_inviter_code' },
    },
    change_password: {
        executor: 'constant-vus',
        vus: 8,
        duration: '5m',
        exec: 'changePasswordScenario',
        tags: { scenario: 'change_password' },
    },
    password_change_token_invalid: {
        executor: 'constant-vus',
        vus: 5,
        duration: '5m',
        exec: 'passwordChangeTokenInvalidScenario',
        tags: { scenario: 'password_change_token_invalid' },
    },
    logout_and_verify: {
        executor: 'constant-vus',
        vus: 5,
        duration: '5m',
        exec: 'logoutScenario',
        tags: { scenario: 'logout_and_verify' },
    },
    invalid_token: {
        executor: 'constant-vus',
        vus: 5,
        duration: '5m',
        exec: 'invalidTokenScenario',
        tags: { scenario: 'invalid_token' },
    },
    edge_cases: {
        executor: 'constant-vus',
        vus: 3,
        duration: '5m',
        exec: 'edgeCasesScenario',
        tags: { scenario: 'edge_cases' },
    },
    mixed_operations: {
        executor: 'constant-vus',
        vus: 10,
        duration: '5m',
        exec: 'mixedOperationsScenario',
        tags: { scenario: 'mixed_operations' },
    },
};

const scenarioVus = {
    'change_nickname': 15,
    'change_password': 8,
    'edge_cases': 3,
    'generate_inviter_code': 10,
    'get_user_info': 20,
    'invalid_token': 5,
    'logout_and_verify': 5,
    'mixed_operations': 10,
    'password_change_token_invalid': 5,
};

let totalVus = 0;
for (const key in scenarioConfigs) {
    if (scenarioConfigs[key].vus) {
        totalVus += scenarioConfigs[key].vus;
    }
}

export const options = {
    scenarios: scenarioConfigs,
    thresholds: {
        'http_req_duration{scenario:get_user_info}': ['p(95)<300', 'p(99)<500'],
        'http_req_failed{scenario:get_user_info}': ['rate<0.01'],
        'http_req_duration{scenario:change_nickname}': ['p(95)<500', 'p(99)<800'],
        'http_req_failed{scenario:change_nickname}': ['rate<0.01'],
        'http_req_duration{scenario:generate_inviter_code}': ['p(95)<300', 'p(99)<500'],
        'http_req_failed{scenario:generate_inviter_code}': ['rate<0.01'],
        'http_req_duration{scenario:change_password}': ['p(95)<500', 'p(99)<1000'],
        'http_req_failed{scenario:change_password}': ['rate<0.01'],
        'http_req_duration{scenario:password_change_token_invalid}': ['p(95)<500', 'p(99)<1000'],
        'http_req_duration{scenario:logout_and_verify}': ['p(95)<300', 'p(99)<500'],
        'http_req_failed{scenario:logout_and_verify}': ['rate<0.01'],
        'http_req_duration{scenario:invalid_token}': ['p(95)<200'],
        'http_req_duration{scenario:edge_cases}': ['p(95)<300'],
        'http_req_duration{scenario:mixed_operations}': ['p(95)<500', 'p(99)<800'],
        'http_req_failed{scenario:mixed_operations}': ['rate<0.01'],
        checks: ['rate>0.95'],
    },
};

export function getUserInfoScenario() {
    const user = getUserByVUAndScenario(__VU, 'get_user_info', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const userInfoResult = getUserInfo(loginResult.userId, loginResult.accessToken, true);

    check(userInfoResult, {
        '获取用户信息成功': (r) => r.success === true,
        '返回用户数据': (r) => r.success && r.userInfo !== undefined,
    });

    randomSleep();
}

export function changeNicknameScenario() {
    const user = getUserByVUAndScenario(__VU, 'change_nickname', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const newNickname = `user_${Date.now()}`;
    const nicknameResult = changeNickname(loginResult.userId, loginResult.accessToken, newNickname, true);

    check(nicknameResult, {
        '修改昵称成功': (r) => r.success === true,
    });

    randomSleep();
}

export function generateInviterCodeScenario() {
    const user = getUserByVUAndScenario(__VU, 'generate_inviter_code', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const inviterCodeResult = generateInviterCode(loginResult.userId, loginResult.accessToken, true);

    check(inviterCodeResult, {
        '生成邀请码成功': (r) => r.success === true,
        '返回邀请码': (r) => r.success && r.inviterCode !== undefined,
    });

    randomSleep(0.3, 0.8);

    const secondInviterCodeResult = generateInviterCode(loginResult.userId, loginResult.accessToken, true);
    check(secondInviterCodeResult, {
        '重复生成邀请码成功': (r) => r.success === true,
    });

    randomSleep();
}

export function changePasswordScenario() {
    const user = getUserByVUAndScenario(__VU, 'change_password', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const originalPassword = user.password;
    const newPassword = generateRandomPassword();
    const changePasswordResult = changePassword(
        loginResult.userId,
        loginResult.accessToken,
        originalPassword,
        newPassword,
        true
    );

    check(changePasswordResult, {
        '修改密码成功': (r) => r.success === true,
    });

    if (changePasswordResult.success) {
        const newLoginResult = login(user.account, newPassword, false);
        if (newLoginResult.success) {
            changePassword(
                newLoginResult.userId,
                newLoginResult.accessToken,
                newPassword,
                originalPassword,
                false,
                true
            );
        } else {
            console.error(`⚠️  密码已修改但无法恢复(新密码登录失败): account=${user.account}, newPassword=${newPassword}`);
        }
    }

    randomSleep();
}

export function passwordChangeTokenInvalidScenario() {
    const user = getUserByVUAndScenario(__VU, 'password_change_token_invalid', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const oldAccessToken = loginResult.accessToken;
    const originalPassword = user.password;
    const newPassword = generateRandomPassword();

    const changePasswordResult = changePassword(
        loginResult.userId,
        oldAccessToken,
        originalPassword,
        newPassword,
        true
    );

    check(changePasswordResult, {
        '修改密码成功': (r) => r.success === true,
    });

    if (!changePasswordResult.success) {
        randomSleep();
        return;
    }

    randomSleep(0.3, 0.8);

    const userInfoResult = getUserInfo(loginResult.userId, oldAccessToken, false);
    check(userInfoResult, {
        '修改密码后旧Token应失效': (r) => r.success === false,
    });

    const newLoginResult = login(user.account, newPassword, true);
    check(newLoginResult, {
        '使用新密码登录成功': (r) => r.success === true,
    });

    if (newLoginResult.success) {
        changePassword(
            newLoginResult.userId,
            newLoginResult.accessToken,
            newPassword,
            originalPassword,
            false,
            true
        );
    } else {
        console.error(`⚠️  密码已修改但无法恢复(新密码登录失败): account=${user.account}, newPassword=${newPassword}`);
    }

    randomSleep();
}

export function logoutScenario() {
    const user = getUserByVUAndScenario(__VU, 'logout_and_verify', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const logoutResult = logout(loginResult.userId, loginResult.accessToken, true);

    check(logoutResult, {
        '登出成功': (r) => r.success === true,
    });

    randomSleep(0.5, 1.0);

    const userInfoResult = getUserInfo(loginResult.userId, loginResult.accessToken, false);
    check(userInfoResult, {
        '登出后获取信息应失败': (r) => r.success === false,
    });

    randomSleep();
}

export function invalidTokenScenario() {
    const user = getUserByVUAndScenario(__VU, 'invalid_token', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const invalidTokenResult = getUserInfo(loginResult.userId, 'invalid_access_token', false);
    check(invalidTokenResult, {
        '无效Token应失败': (r) => r.success === false,
    });

    randomSleep();
}

export function edgeCasesScenario() {
    const user = getUserByVUAndScenario(__VU, 'edge_cases', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const emptyNicknameResult = changeNickname(loginResult.userId, loginResult.accessToken, '', false);
    check(emptyNicknameResult, {
        '空昵称应失败': (r) => r.success === false,
    });

    randomSleep(0.3, 0.8);

    const longNicknameResult = changeNickname(
        loginResult.userId,
        loginResult.accessToken,
        'a'.repeat(1000),
        false
    );
    check(longNicknameResult, {
        '超长昵称应失败': (r) => r.success === false,
    });

    randomSleep();
}

export function mixedOperationsScenario() {
    const user = getUserByVUAndScenario(__VU, 'mixed_operations', scenarioVus);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const userInfoResult = getUserInfo(loginResult.userId, loginResult.accessToken, true);
    check(userInfoResult, {
        '获取用户信息成功': (r) => r.success === true,
    });

    randomSleep(0.2, 0.5);

    const inviterCodeResult = generateInviterCode(loginResult.userId, loginResult.accessToken, true);
    check(inviterCodeResult, {
        '生成邀请码成功': (r) => r.success === true,
    });

    randomSleep(0.2, 0.5);

    const newNickname = `user_${Date.now()}`;
    const nicknameResult = changeNickname(loginResult.userId, loginResult.accessToken, newNickname, true);
    check(nicknameResult, {
        '修改昵称成功': (r) => r.success === true,
    });

    randomSleep(0.2, 0.5);

    const userInfoResult2 = getUserInfo(loginResult.userId, loginResult.accessToken, true);
    check(userInfoResult2, {
        '再次获取用户信息成功': (r) => r.success === true,
    });

    randomSleep();
}
