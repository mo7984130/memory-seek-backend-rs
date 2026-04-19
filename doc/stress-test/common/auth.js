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
        timeout: 60 * 1000,
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
    const traceId = res.headers['X-Trace-Id'] || res.headers['x-trace-id'] || 'N/A';

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ 登录失败(预期成功): account=${account}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
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
            console.error(`❌ JSON解析失败: account=${account}`);
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
    const traceId = res.headers['X-Trace-Id'] || res.headers['x-trace-id'] || 'N/A';

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ Token刷新失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
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

export function getAuthHeaders(userId, accessToken) {
    if (!userId || !accessToken) {
        throw new Error('userId和accessToken不能为空');
    }
    return {
        'Content-Type': 'application/json',
        'Authorization': `${userId} ${accessToken}`,
    };
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
        normal_login: {
            executor: 'constant-vus',
            vus: 30,
            duration: '5m',
            exec: 'normalLoginScenario',
            tags: { scenario: 'normal_login' },
        },
        wrong_password: {
            executor: 'constant-vus',
            vus: 5,
            duration: '5m',
            exec: 'wrongPasswordScenario',
            tags: { scenario: 'wrong_password' },
        },
        user_not_found: {
            executor: 'constant-vus',
            vus: 3,
            duration: '5m',
            exec: 'userNotFoundScenario',
            tags: { scenario: 'user_not_found' },
        },
        token_refresh: {
            executor: 'constant-vus',
            vus: 8,
            duration: '5m',
            exec: 'tokenRefreshScenario',
            tags: { scenario: 'token_refresh' },
        },
        invalid_token_refresh: {
            executor: 'constant-vus',
            vus: 2,
            duration: '5m',
            exec: 'invalidTokenRefreshScenario',
            tags: { scenario: 'invalid_token_refresh' },
        },
        empty_credentials: {
            executor: 'constant-vus',
            vus: 2,
            duration: '5m',
            exec: 'emptyCredentialsScenario',
            tags: { scenario: 'empty_credentials' },
        },
    },
    thresholds: {
        'http_req_duration{scenario:normal_login}': ['p(95)<500', 'p(99)<1000'],
        'http_req_failed{scenario:normal_login}': ['rate<0.01'],
        'http_req_duration{scenario:wrong_password}': ['p(95)<300'],
        'http_req_failed{scenario:wrong_password}': ['rate<0.01'],
        'http_req_duration{scenario:user_not_found}': ['p(95)<300'],
        'http_req_failed{scenario:user_not_found}': ['rate<0.01'],
        'http_req_duration{scenario:token_refresh}': ['p(95)<300', 'p(99)<500'],
        'http_req_failed{scenario:token_refresh}': ['rate<0.01'],
        'http_req_duration{scenario:invalid_token_refresh}': ['p(95)<200'],
        'http_req_duration{scenario:empty_credentials}': ['p(95)<200'],
        checks: ['rate>0.95'],
    },
};

export function normalLoginScenario() {
    const user = getRandomUser();
    const result = login(user.account, user.password, true);

    check(result, {
        '正常登录成功': (r) => r.success === true,
        '返回有效Token': (r) => r.success && r.accessToken !== undefined,
    });

    randomSleep();
}

export function wrongPasswordScenario() {
    const user = getRandomUser();
    const result = login(user.account, 'wrong_password_123', false);

    check(result, {
        '密码错误应失败': (r) => r.success === false,
    });

    randomSleep();
}

export function userNotFoundScenario() {
    const account = `nonexistent_user_${__VU}_${Date.now()}`;
    const result = login(account, 'any_password', false);

    check(result, {
        '用户不存在应失败': (r) => r.success === false,
    });

    randomSleep();
}

export function tokenRefreshScenario() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (loginResult.success) {
        const newAccessToken = refreshAccessToken(loginResult.userId, loginResult.refreshToken, true);

        check(newAccessToken, {
            'Token刷新成功': (token) => token !== null,
        });
    }

    randomSleep();
}

export function invalidTokenRefreshScenario() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];
    const loginResult = login(user.account, user.password, true);

    if (loginResult.success) {
        const invalidRefreshResult = refreshAccessToken(loginResult.userId, 'invalid_refresh_token', false);

        check(invalidRefreshResult, {
            '无效Token刷新应失败': (token) => token === null,
        });
    }

    randomSleep();
}

export function emptyCredentialsScenario() {
    const scenario = Math.random();
    let account, password, scenarioName;

    if (scenario < 0.5) {
        account = '';
        password = 'any_password';
        scenarioName = '空账号';
    } else {
        const user = getRandomUser();
        account = user.account;
        password = '';
        scenarioName = '空密码';
    }

    const result = login(account, password, false);

    check(result, {
        '空凭证应失败': (r) => r.success === false,
    });

    randomSleep();
}
