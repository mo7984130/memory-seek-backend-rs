import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from './config.js';
import {
    ErrorTypes,
    parseJsonSafely,
    makeRequest,
    createErrorResult,
    createSuccessResult,
    logPerformance,
} from './utils.js';

export function login(account, password, expectedSuccess = true) {
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
        if (expectedSuccess) {
            console.error(`❌ 登录请求异常: account=${account}, error=${e.message}`);
        }
        return createErrorResult(ErrorTypes.REQUEST_FAILED, {
            message: e.message,
            account,
        });
    }

    const duration = res.timings.duration;
    const isSuccess = res.status === 200;
    
    if (expectedSuccess && !isSuccess) {
        console.error(`❌ 登录失败(预期成功): account=${account}, status=${res.status}, duration=${duration}ms, body=${res.body.substring(0, 200)}`);
        logPerformance('login', duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  登录意外成功(预期失败): account=${account}, status=${res.status}`);
    }
    
    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            account,
        });
    }

    const body = parseJsonSafely(res.body, 'login');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ JSON解析失败: account=${account}, body=${res.body.substring(0, 200)}`);
        }
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

export function refreshAccessToken(userId, refreshToken, expectedSuccess = true) {
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
        if (expectedSuccess) {
            console.error(`❌ Token刷新请求异常: userId=${userId}, error=${e.message}`);
        }
        return null;
    }

    const duration = res.timings.duration;
    const isSuccess = res.status === 200;
    
    if (expectedSuccess && !isSuccess) {
        console.error(`❌ Token刷新失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, body=${res.body ? res.body.substring(0, 200) : 'empty'}`);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  Token刷新意外成功(预期失败): userId=${userId}`);
    }

    if (!isSuccess) {
        return null;
    }

    const body = parseJsonSafely(res.body, 'refreshToken');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ Token刷新JSON解析失败: userId=${userId}`);
        }
        return null;
    }

    logPerformance('refreshToken', duration, true);
    return body.data?.accessToken;
}

function getRandomUser() {
    const userIndex = Math.floor(Math.random() * config.testUsers.length);
    return config.testUsers[userIndex];
}

function randomSleep(min = 0.5, max = 1.5) {
    sleep(Math.random() * (max - min) + min);
}

export const options = {
    scenarios: {
        baseline_test: {
            executor: 'constant-vus',
            vus: 1,
            duration: '2m',
            exec: 'baselineScenario',
            tags: { scenario: 'baseline' },
        },
    },
    thresholds: {
        'http_req_duration{scenario:baseline}': ['p(99)<200'],
        'http_req_failed{scenario:baseline}': ['rate<0.01'],
        checks: ['rate>0.99'],
    },
};

export function baselineScenario() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];
    
    console.log(`测试用户: ${user.account}`);
    
    const loginResult = login(user.account, user.password, true);
    
    check(loginResult, {
        '登录成功': (r) => r.success === true,
        '返回userId': (r) => r.success && r.userId !== undefined,
        '返回accessToken': (r) => r.success && r.accessToken !== undefined,
        '返回refreshToken': (r) => r.success && r.refreshToken !== undefined,
    });
    
    if (!loginResult.success) {
        console.error(`❌ 基准测试失败: 无法登录用户 ${user.account}`);
        console.error(`错误详情: ${JSON.stringify(loginResult)}`);
    } else {
        console.log(`✅ 登录成功: userId=${loginResult.userId}`);
        
        const newToken = refreshAccessToken(loginResult.userId, loginResult.refreshToken, true);
        
        check(newToken, {
            'Token刷新成功': (token) => token !== null,
        });
        
        if (newToken) {
            console.log(`✅ Token刷新成功`);
        } else {
            console.error(`❌ Token刷新失败`);
        }
    }
    
    randomSleep(2, 3);
}
