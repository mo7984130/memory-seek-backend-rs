import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from '../common/config.js';
import {getAuthHeaders, login} from '../common/auth.js';
import {
    ErrorTypes,
    parseJsonSafely,
    makeRequest,
    createErrorResult,
    createSuccessResult,
    logPerformance,
} from '../common/utils.js';

export function getPhotoCursor(headers, cursor = null, size = 20, direction = 'next') {
    const url = `${config.baseUrl}/photo/photo/cursor`;
    const params = {
        headers,
        timeout: config.timeout,
    };

    const queryParams = {
        cursor,
        size,
        direction,
    };

    let res;
    try {
        res = http.get(url, Object.assign({}, params, { params: queryParams }));
    } catch (e) {
        console.error(`获取照片游标分页失败: ${e.message}`);
        return createErrorResult(ErrorTypes.REQUEST_FAILED, {
            message: e.message,
        });
    }

    const duration = res.timings.duration;
    const checks = check(res, {
        '照片游标分页状态码为200': (r) => r.status === 200,
        '照片游标分页响应时间<500ms': (r) => r.timings.duration < 500,
        '照片游标分页返回数据': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.code === 200 && Array.isArray(body.data?.records);
            } catch {
                return false;
            }
        },
    });

    if (!checks) {
        logPerformance('getPhotoCursor', duration, false);
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
        });
    }

    const body = parseJsonSafely(res.body, 'getPhotoCursor');
    if (!body) {
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
        });
    }

    logPerformance('getPhotoCursor', duration, true);
    return createSuccessResult({
        records: body.data?.records || [],
        nextCursor: body.data?.nextCursor,
        hasMore: body.data?.hasMore,
    });
}

export function checkMd5Exists(headers, md5) {
    const url = `${config.baseUrl}/photo/photo/md5-exist`;
    const params = {
        headers,
        timeout: config.timeout,
    };

    const queryParams = {
        md5,
    };

    let res;
    try {
        res = http.get(url, Object.assign({}, params, { params: queryParams }));
    } catch (e) {
        console.error(`检查MD5是否存在失败: ${e.message}`);
        return createErrorResult(ErrorTypes.REQUEST_FAILED, {
            message: e.message,
            md5,
        });
    }

    const duration = res.timings.duration;
    const checks = check(res, {
        '检查MD5状态码为200': (r) => r.status === 200,
        '检查MD5响应时间<300ms': (r) => r.timings.duration < 300,
        '检查MD5返回数据': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.code === 200 && typeof body.data === 'boolean';
            } catch {
                return false;
            }
        },
    });

    if (!checks) {
        logPerformance('checkMd5Exists', duration, false);
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            md5,
        });
    }

    const body = parseJsonSafely(res.body, 'checkMd5Exists');
    if (!body) {
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            md5,
        });
    }

    logPerformance('checkMd5Exists', duration, true);
    return createSuccessResult({
        exists: body.data,
    });
}

export function getPhotoTimeRange(headers) {
    const url = `${config.baseUrl}/photo/photo/time-range`;
    const params = {
        headers,
        timeout: config.timeout,
    };

    let res;
    try {
        res = http.get(url, params);
    } catch (e) {
        console.error(`获取照片时间范围失败: ${e.message}`);
        return createErrorResult(ErrorTypes.REQUEST_FAILED, {
            message: e.message,
        });
    }

    const duration = res.timings.duration;
    const checks = check(res, {
        '获取时间范围状态码为200': (r) => r.status === 200,
        '获取时间范围响应时间<300ms': (r) => r.timings.duration < 300,
        '获取时间范围返回数据': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.code === 200 && body.data?.min && body.data?.max;
            } catch {
                return false;
            }
        },
    });

    if (!checks) {
        logPerformance('getPhotoTimeRange', duration, false);
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
        });
    }

    const body = parseJsonSafely(res.body, 'getPhotoTimeRange');
    if (!body) {
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
        });
    }

    logPerformance('getPhotoTimeRange', duration, true);
    return createSuccessResult({
        min: body.data?.min,
        max: body.data?.max,
    });
}

export const options = {
    stages: [
        { duration: '30s', target: 10 },
        { duration: '1m', target: 50 },
        { duration: '3m', target: 50 },
        { duration: '30s', target: 0 },
    ],
    thresholds: config.thresholds,
};

export default function () {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    const authResult = login(user.account, user.password);
    if (!authResult.success) {
        console.log(`登录失败: ${user.account}`);
        sleep(1);
        return;
    }

    const headers = getAuthHeaders(authResult.userId, authResult.accessToken);

    const timeRangeResult = getPhotoTimeRange(headers);
    sleep(0.5);

    const testMd5Values = [
        'd41d8cd98f00b204e9800998ecf8427e',
        '098f6bcd4621d373cade4e832627b4f6',
        'e99a18c428cb38d5f260853678922e03',
        '5d41402abc4b2a76b9719d911017c592',
        'acbd18db4cc2f85cedef654fccc4a4d8',
    ];

    const randomMd5 = testMd5Values[Math.floor(Math.random() * testMd5Values.length)];
    checkMd5Exists(headers, randomMd5);
    sleep(0.5);

    let cursor = null;
    for (let i = 0; i < 5; i++) {
        const cursorResult = getPhotoCursor(headers, cursor, 20, 'next');

        if (cursorResult.success) {
            cursor = cursorResult.nextCursor;
            if (!cursorResult.hasMore) break;
        } else {
            break;
        }

        sleep(1);
    }

    sleep(2);
}
