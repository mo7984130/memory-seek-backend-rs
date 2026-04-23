import { check, sleep } from 'k6';
import { config } from '../common/config.js';
import { login } from '../common/auth.js';
import {
    getRandomUser,
    getUserByVU,
    randomSleep,
    validateUserCount,
} from '../common/skill.js';
import {
    getPhotoCursor,
    checkMd5Exists,
    getPhotoTimeRange,
} from './photo_service.js';

/**
 * ==================== Photo 模块压测场景 ====================
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
 */

const scenarioConfigs = {
    get_time_range: {
        executor: 'constant-vus',
        vus: 15,
        duration: '5m',
        exec: 'getTimeRangeScenario',
        tags: { scenario: 'get_time_range' },
    },
    check_md5: {
        executor: 'constant-vus',
        vus: 20,
        duration: '5m',
        exec: 'checkMd5Scenario',
        tags: { scenario: 'check_md5' },
    },
    cursor_pagination: {
        executor: 'constant-vus',
        vus: 25,
        duration: '5m',
        exec: 'cursorPaginationScenario',
        tags: { scenario: 'cursor_pagination' },
    },
    invalid_token: {
        executor: 'constant-vus',
        vus: 5,
        duration: '5m',
        exec: 'invalidTokenScenario',
        tags: { scenario: 'invalid_token' },
    },
    mixed_operations: {
        executor: 'constant-vus',
        vus: 15,
        duration: '5m',
        exec: 'mixedOperationsScenario',
        tags: { scenario: 'mixed_operations' },
    },
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
        'http_req_duration{scenario:get_time_range}': ['p(95)<300', 'p(99)<500'],
        'http_req_failed{scenario:get_time_range}': ['rate<0.01'],
        'http_req_duration{scenario:check_md5}': ['p(95)<300', 'p(99)<500'],
        'http_req_failed{scenario:check_md5}': ['rate<0.01'],
        'http_req_duration{scenario:cursor_pagination}': ['p(95)<500', 'p(99)<800'],
        'http_req_failed{scenario:cursor_pagination}': ['rate<0.01'],
        'http_req_duration{scenario:invalid_token}': ['p(95)<200'],
        'http_req_duration{scenario:mixed_operations}': ['p(95)<500', 'p(99)<800'],
        'http_req_failed{scenario:mixed_operations}': ['rate<0.01'],
        checks: ['rate>0.95'],
    },
};

const testMd5Values = [
    'd41d8cd98f00b204e9800998ecf8427e',
    '098f6bcd4621d373cade4e832627b4f6',
    'e99a18c428cb38d5f260853678922e03',
    '5d41402abc4b2a76b9719d911017c592',
    'acbd18db4cc2f85cedef654fccc4a4d8',
];

export function setup() {
    validateUserCount(totalVus);
}

export function getTimeRangeScenario() {
    const user = getUserByVU(__VU);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const timeRangeResult = getPhotoTimeRange(loginResult.userId, loginResult.accessToken, true);

    check(timeRangeResult, {
        '获取时间范围成功': (r) => r.success === true,
        '返回min和max': (r) => r.success && r.min !== undefined && r.max !== undefined,
    });

    randomSleep();
}

export function checkMd5Scenario() {
    const user = getUserByVU(__VU);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const randomMd5 = testMd5Values[Math.floor(Math.random() * testMd5Values.length)];
    const md5Result = checkMd5Exists(loginResult.userId, loginResult.accessToken, randomMd5, true);

    check(md5Result, {
        '检查MD5成功': (r) => r.success === true,
        '返回exists字段': (r) => r.success && r.exists !== undefined,
    });

    randomSleep();
}

export function cursorPaginationScenario() {
    const user = getUserByVU(__VU);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    let cursor = null;
    let pageCount = 0;
    const maxPages = 5;

    for (let i = 0; i < maxPages; i++) {
        const cursorResult = getPhotoCursor(loginResult.userId, loginResult.accessToken, cursor, 20, 'next', true);

        if (cursorResult.success) {
            pageCount++;
            cursor = cursorResult.nextCursor;

            check(cursorResult, {
                '游标分页成功': (r) => r.success === true,
                '返回records数组': (r) => r.success && Array.isArray(r.records),
            });

            if (!cursorResult.hasMore) break;
        } else {
            break;
        }

        sleep(0.5);
    }

    check({ pageCount }, {
        '至少获取一页数据': (r) => r.pageCount >= 1,
    });

    randomSleep();
}

export function invalidTokenScenario() {
    const user = getUserByVU(__VU);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const invalidTokenResult = getPhotoTimeRange(loginResult.userId, 'invalid_access_token', false);
    check(invalidTokenResult, {
        '无效Token应失败': (r) => r.success === false,
    });

    randomSleep(0.3, 0.8);

    const invalidMd5Result = checkMd5Exists(loginResult.userId, 'invalid_access_token', testMd5Values[0], false);
    check(invalidMd5Result, {
        '无效Token检查MD5应失败': (r) => r.success === false,
    });

    randomSleep();
}

export function mixedOperationsScenario() {
    const user = getUserByVU(__VU);
    const loginResult = login(user.account, user.password, true);

    check(loginResult, {
        '登录成功': (r) => r.success === true,
    });

    if (!loginResult.success) {
        randomSleep();
        return;
    }

    const timeRangeResult = getPhotoTimeRange(loginResult.userId, loginResult.accessToken, true);
    check(timeRangeResult, {
        '获取时间范围成功': (r) => r.success === true,
    });

    randomSleep(0.2, 0.5);

    const randomMd5 = testMd5Values[Math.floor(Math.random() * testMd5Values.length)];
    const md5Result = checkMd5Exists(loginResult.userId, loginResult.accessToken, randomMd5, true);
    check(md5Result, {
        '检查MD5成功': (r) => r.success === true,
    });

    randomSleep(0.2, 0.5);

    const cursorResult = getPhotoCursor(loginResult.userId, loginResult.accessToken, null, 10, 'next', true);
    check(cursorResult, {
        '游标分页成功': (r) => r.success === true,
    });

    randomSleep();
}
