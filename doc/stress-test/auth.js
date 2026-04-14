import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from './config.js';
import {
    ErrorTypes,
    PerformanceThresholds,
    parseJsonSafely,
    makeRequest,
    createErrorResult,
    createSuccessResult,
    logPerformance,
} from './utils.js';

export function login(account, password) {
    const url = `${config.baseUrl}/auth/login`;
    const payload = JSON.stringify({ account, password });
    const params = {
        headers: { 'Content-Type': 'application/json' },
        timeout: config.timeout,
    };

    let res;
    try {
        res = makeRequest('POST', url, payload, params);
    } catch (e) {
        console.error(`登录请求失败: ${e.message}`);
        return createErrorResult(ErrorTypes.REQUEST_FAILED, {
            message: e.message,
            account,
        });
    }

    const duration = res.timings.duration;
    const checks = check(res, {
        '登录状态码为200': (r) => r.status === 200,
    });

    if (!checks) {
        logPerformance('login', duration, false);
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            account,
        });
    }

    const body = parseJsonSafely(res.body, 'login');
    if (!body) {
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            account,
        });
    }

    logPerformance('login', duration, true);
    return createSuccessResult({
        userId: body.data?.id,
        accessToken: body.data?.accessToken,
        refreshToken: body.data?.refreshToken,
        username: body.data?.username,
    });
}

export function refreshAccessToken(userId, refreshToken) {
    const url = `${config.baseUrl}/auth/access-token`;
    const params = {
        headers: {
            'X-User-Id': userId,
            'X-Refresh-Token': refreshToken,
        },
        timeout: config.timeout,
    };

    let res;
    try {
        res = makeRequest('GET', url, null, params);
    } catch (e) {
        console.error(`刷新Token请求失败: ${e.message}`);
        return null;
    }

    const duration = res.timings.duration;
    const checks = check(res, {
        '刷新Token状态码为200': (r) => r.status === 200,
    });

    if (!checks || res.status !== 200) {
        console.error(`刷新Token失败: status=${res.status}, duration=${duration}ms`);
        return null;
    }

    const body = parseJsonSafely(res.body, 'refreshToken');
    if (!body) {
        logPerformance('refreshToken', duration, false);
        return null;
    }

    logPerformance('refreshToken', duration, true);
    return body.data?.accessToken;
}

export function getAuthHeaders(userId, accessToken) {
    if (!userId || !accessToken) {
        throw new Error('userId和accessToken不能为空');
    }
    return {
        'Content-Type': 'application/json',
        'Authorization': `${userId} ${accessToken}`,
    };
}

export const options = {
    stages: [
        { duration: '10s', target: 50 },
        { duration: '5m', target: 50 },
        { duration: '10s', target: 0 },
        // { duration: '30s', target: 0 },
    ],
    thresholds: config.thresholds,
};

export default function () {
    const scenario = Math.random();
    
    let account, password, expectedSuccess, scenarioName;
    
    if (scenario < 0.5) {
        const userIndex = (__VU - 1) % config.testUsers.length;
        const user = config.testUsers[userIndex];
        account = user.account;
        password = user.password;
        expectedSuccess = true;
        scenarioName = '正确登录';
    } else if (scenario < 0.6) {
        const userIndex = (__VU - 1) % config.testUsers.length;
        const user = config.testUsers[userIndex];
        account = user.account;
        password = 'wrong_password_123';
        expectedSuccess = false;
        scenarioName = '密码错误';
    } else if (scenario < 0.7) {
        account = `nonexistent_user_${__VU}_${Date.now()}`;
        password = 'any_password';
        expectedSuccess = false;
        scenarioName = '用户不存在';
    } else if (scenario < 0.75) {
        const userIndex = (__VU - 1) % config.testUsers.length;
        const user = config.testUsers[userIndex];
        account = user.account;
        password = '';
        expectedSuccess = false;
        scenarioName = '空密码';
    } else if (scenario < 0.8) {
        account = '';
        password = 'any_password';
        expectedSuccess = false;
        scenarioName = '空账号';
    } else if (scenario < 0.85) {
        account = 'a'.repeat(1000);
        password = 'any_password';
        expectedSuccess = false;
        scenarioName = '超长账号';
    } else if (scenario < 0.9) {
        account = `user_${__VU}_${Date.now()}`;
        password = `user_${__VU}_${Date.now()}`;
        expectedSuccess = false;
        scenarioName = '随机账号密码';
    } else if (scenario < 0.95) {
        const userIndex = (__VU - 1) % config.testUsers.length;
        const user = config.testUsers[userIndex];
        account = user.account;
        password = user.password;
        expectedSuccess = true;
        scenarioName = '重复登录';
    } else {
        const userIndex = (__VU - 1) % config.testUsers.length;
        const user = config.testUsers[userIndex];
        account = user.account;
        password = user.password;
        expectedSuccess = true;
        scenarioName = '并发登录';
    }
    
    const loginResult = login(account, password);
    
    check(loginResult, {
        '登录结果符合预期': (r) => r.success === expectedSuccess,
    });
    
    if (expectedSuccess && !loginResult.success) {
        console.error(`VU ${__VU} [${scenarioName}] 预期成功但失败: ${JSON.stringify(loginResult)}`);
    }
    
    if (!expectedSuccess && loginResult.success) {
        console.error(`VU ${__VU} [${scenarioName}] 预期失败但成功: ${JSON.stringify(loginResult)}`);
    }
    
    if (loginResult.success) {
        sleep(0.5);
        
        if (scenario >= 0.95) {
            const secondLoginResult = login(account, password);
            check(secondLoginResult, {
                '并发登录成功': (r) => r.success === true,
            });
            sleep(0.5);
        }
        
        const newAccessToken = refreshAccessToken(loginResult.userId, loginResult.refreshToken);
        
        check(newAccessToken, {
            'Token刷新成功': (token) => token !== null,
        });
        
        if (!newAccessToken) {
            console.error(`VU ${__VU} Token刷新失败`);
        }
        
        sleep(0.5);
        
        const invalidRefreshResult = refreshAccessToken(loginResult.userId, 'invalid_refresh_token');
        check(invalidRefreshResult, {
            '无效Token刷新应失败': (token) => token === null,
        });
    }
    
    sleep(0.5);
}
