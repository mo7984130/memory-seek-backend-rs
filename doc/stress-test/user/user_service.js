import http from 'k6/http';
import { check, sleep } from 'k6';
import { config } from '../common/config.js';
import {
    ErrorTypes,
    parseJsonSafely,
    makeRequest,
    createErrorResult,
    createSuccessResult,
    logPerformance,
} from '../common/utils.js';
import { getAuthHeaders } from '../common/auth.js';
import {
    extractTraceId,
    validateResponse,
    parseAndValidate,
    handleRequestError,
} from '../common/skill.js';
import { generateRandomPassword } from '../utils/password.js';

export function getUserInfo(userId, accessToken, expectedSuccess = true) {
    const url = `${config.baseUrl}/user/info`;
    const params = {
        headers: getAuthHeaders(userId, accessToken),
        timeout: config.timeout,
    };

    let res;
    try {
        res = makeRequest('GET', url, null, params);
    } catch (e) {
        return handleRequestError('获取用户信息', { userId }, e, expectedSuccess);
    }

    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const isSuccess = res.status === 200;

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ 获取用户信息失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance('getUserInfo', duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  获取用户信息意外成功(预期失败): userId=${userId}, status=${res.status}`);
    }

    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            userId,
        });
    }

    const body = parseJsonSafely(res.body, 'getUserInfo');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ 获取用户信息JSON解析失败: userId=${userId}`);
        }
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            userId,
        });
    }

    logPerformance('getUserInfo', duration, true);
    return createSuccessResult({
        userInfo: body.data,
    });
}

export function changeNickname(userId, accessToken, newNickname, expectedSuccess = true) {
    const url = `${config.baseUrl}/user/nickname`;
    const payload = JSON.stringify({ newNickname: newNickname });
    const params = {
        headers: getAuthHeaders(userId, accessToken),
        timeout: config.timeout,
    };

    let res;
    try {
        res = makeRequest('POST', url, payload, params);
    } catch (e) {
        return handleRequestError('修改昵称', { userId }, e, expectedSuccess);
    }

    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const isSuccess = res.status === 200;

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ 修改昵称失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance('changeNickname', duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  修改昵称意外成功(预期失败): userId=${userId}, status=${res.status}`);
    }

    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            userId,
        });
    }

    const body = parseJsonSafely(res.body, 'changeNickname');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ 修改昵称JSON解析失败: userId=${userId}`);
        }
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            userId,
        });
    }

    logPerformance('changeNickname', duration, true);
    return createSuccessResult({
        nickname: body.data,
    });
}

export function generateInviterCode(userId, accessToken, expectedSuccess = true) {
    const url = `${config.baseUrl}/user/inviter-code`;
    const params = {
        headers: getAuthHeaders(userId, accessToken),
        timeout: config.timeout,
    };

    let res;
    try {
        res = makeRequest('GET', url, null, params);
    } catch (e) {
        return handleRequestError('生成邀请码', { userId }, e, expectedSuccess);
    }

    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const isSuccess = res.status === 200;

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ 生成邀请码失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance('generateInviterCode', duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  生成邀请码意外成功(预期失败): userId=${userId}, status=${res.status}`);
    }

    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            userId,
        });
    }

    const body = parseJsonSafely(res.body, 'generateInviterCode');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ 生成邀请码JSON解析失败: userId=${userId}`);
        }
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            userId,
        });
    }

    logPerformance('generateInviterCode', duration, true);
    return createSuccessResult({
        inviterCode: body.data?.inviterCode,
        expireAt: body.data?.expireAt,
    });
}

export function changePassword(userId, accessToken, oldPassword, newPassword, expectedSuccess = true, isCleanup = false) {
    const url = `${config.baseUrl}/user/password`;
    const payload = JSON.stringify({
        oldPassword: oldPassword,
        newPassword: newPassword,
    });
    const params = {
        headers: getAuthHeaders(userId, accessToken),
        timeout: config.timeout,
    };

    let res;
    try {
        res = makeRequest('POST', url, payload, params);
    } catch (e) {
        return handleRequestError('修改密码', { userId }, e, expectedSuccess);
    }

    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const isSuccess = res.status === 200;

    if (isCleanup) {
        if (!isSuccess) {
            console.warn(`⚠️  密码恢复失败(清理操作): userId=${userId}, status=${res.status}, duration=${duration}ms`);
        }
    } else {
        if (expectedSuccess && !isSuccess) {
            const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
            console.error(`❌ 修改密码失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
            logPerformance('changePassword', duration, false);
        } else if (!expectedSuccess && isSuccess) {
            console.error(`⚠️  修改密码意外成功(预期失败): userId=${userId}, status=${res.status}`);
        }
    }

    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            userId,
        });
    }

    const body = parseJsonSafely(res.body, 'changePassword');
    if (!body) {
        if (expectedSuccess && !isCleanup) {
            console.error(`❌ 修改密码JSON解析失败: userId=${userId}`);
        }
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            userId,
        });
    }

    if (!isCleanup) {
        logPerformance('changePassword', duration, true);
    }
    return createSuccessResult({});
}

export function logout(userId, accessToken, expectedSuccess = true) {
    const url = `${config.baseUrl}/user/logout`;
    const params = {
        headers: getAuthHeaders(userId, accessToken),
        timeout: config.timeout,
    };

    let res;
    try {
        res = makeRequest('POST', url, null, params);
    } catch (e) {
        return handleRequestError('登出', { userId }, e, expectedSuccess);
    }

    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const isSuccess = res.status === 200;

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ 登出失败(预期成功): userId=${userId}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance('logout', duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  登出意外成功(预期失败): userId=${userId}, status=${res.status}`);
    }

    if (!isSuccess) {
        return createErrorResult(ErrorTypes.CHECK_FAILED, {
            status: res.status,
            duration,
            userId,
        });
    }

    const body = parseJsonSafely(res.body, 'logout');
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ 登出JSON解析失败: userId=${userId}`);
        }
        return createErrorResult(ErrorTypes.INVALID_JSON, {
            status: res.status,
            userId,
        });
    }

    logPerformance('logout', duration, true);
    return createSuccessResult({});
}
