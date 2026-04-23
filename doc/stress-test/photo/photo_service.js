import http from 'k6/http';
import { check, sleep } from 'k6';
import { config } from '../common/config.js';
import { getAuthHeaders, login } from '../common/auth.js';
import {
    ErrorTypes,
    parseJsonSafely,
    makeRequest,
    createErrorResult,
    createSuccessResult,
    logPerformance,
} from '../common/utils.js';
import {
    extractTraceId,
    validateResponse,
    parseAndValidate,
    handleRequestError,
    getRandomUser,
    getUserByVU,
    randomSleep,
} from '../common/skill.js';

export function getPhotoCursor(userId, accessToken, cursor = null, size = 20, direction = 'next', expectedSuccess = true) {
    const url = `${config.baseUrl}/photo/photo/cursor`;
    const params = {
        headers: getAuthHeaders(userId, accessToken),
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
        return handleRequestError('获取照片游标分页', { userId }, e, expectedSuccess);
    }

    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const isSuccess = res.status === 200;

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ 获取照片游标分页失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance('getPhotoCursor', duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  获取照片游标分页意外成功(预期失败): userId=${userId}, status=${res.status}`);
    }

    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            userId,
        });
    }

    const body = parseJsonSafely(res.body, 'getPhotoCursor');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ 获取照片游标分页JSON解析失败: userId=${userId}`);
        }
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            userId,
        });
    }

    logPerformance('getPhotoCursor', duration, true);
    return createSuccessResult({
        records: body.data?.records || [],
        nextCursor: body.data?.nextCursor,
        hasMore: body.data?.hasMore,
    });
}

export function checkMd5Exists(userId, accessToken, md5, expectedSuccess = true) {
    const url = `${config.baseUrl}/photo/photo/md5-exist`;
    const params = {
        headers: getAuthHeaders(userId, accessToken),
        timeout: config.timeout,
    };

    const queryParams = {
        md5,
    };

    let res;
    try {
        res = http.get(url, Object.assign({}, params, { params: queryParams }));
    } catch (e) {
        return handleRequestError('检查MD5是否存在', { userId, md5 }, e, expectedSuccess);
    }

    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const isSuccess = res.status === 200;

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ 检查MD5是否存在失败(预期成功): userId=${userId}, md5=${md5}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance('checkMd5Exists', duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  检查MD5是否存在意外成功(预期失败): userId=${userId}, md5=${md5}, status=${res.status}`);
    }

    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            md5,
            userId,
        });
    }

    const body = parseJsonSafely(res.body, 'checkMd5Exists');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ 检查MD5是否存在JSON解析失败: userId=${userId}, md5=${md5}`);
        }
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            md5,
            userId,
        });
    }

    logPerformance('checkMd5Exists', duration, true);
    return createSuccessResult({
        exists: body.data,
    });
}

export function getPhotoTimeRange(userId, accessToken, expectedSuccess = true) {
    const url = `${config.baseUrl}/photo/photo/time-range`;
    const params = {
        headers: getAuthHeaders(userId, accessToken),
        timeout: config.timeout,
    };

    let res;
    try {
        res = http.get(url, params);
    } catch (e) {
        return handleRequestError('获取照片时间范围', { userId }, e, expectedSuccess);
    }

    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const isSuccess = res.status === 200;

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ 获取照片时间范围失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance('getPhotoTimeRange', duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  获取照片时间范围意外成功(预期失败): userId=${userId}, status=${res.status}`);
    }

    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            userId,
        });
    }

    const body = parseJsonSafely(res.body, 'getPhotoTimeRange');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ 获取照片时间范围JSON解析失败: userId=${userId}`);
        }
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            userId,
        });
    }

    logPerformance('getPhotoTimeRange', duration, true);
    return createSuccessResult({
        min: body.data?.min,
        max: body.data?.max,
    });
}
