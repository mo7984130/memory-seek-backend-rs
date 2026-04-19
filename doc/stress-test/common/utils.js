import http from 'k6/http';
import { sleep } from 'k6';

export const ErrorTypes = {
    REQUEST_FAILED: 'REQUEST_FAILED',
    CHECK_FAILED: 'CHECK_FAILED',
    INVALID_JSON: 'INVALID_JSON',
    BUSINESS_ERROR: 'BUSINESS_ERROR',
    TIMEOUT: 'TIMEOUT',
};

export const PerformanceThresholds = {
    login: {
        responseTime: 500,
        successRate: 0.95,
    },
    refreshToken: {
        responseTime: 300,
        successRate: 0.99,
    },
    getUserInfo: {
        responseTime: 200,
        successRate: 0.99,
    },
    changeNickname: {
        responseTime: 300,
        successRate: 0.95,
    },
    generateInviterCode: {
        responseTime: 200,
        successRate: 0.99,
    },
    changePassword: {
        responseTime: 500,
        successRate: 0.95,
    },
    logout: {
        responseTime: 200,
        successRate: 0.99,
    },
};

export const RetryConfig = {
    maxRetries: 2,
    backoffBase: 0.5,
    retryableStatusCodes: [500, 502, 503, 504],
};

export function parseJsonSafely(body, context = '') {
    try {
        return JSON.parse(body);
    } catch (e) {
        console.error(`[${context}] JSON解析失败: ${e.message}, body: ${body?.substring(0, 100)}`);
        return null;
    }
}

export function makeRequest(method, url, payload, params, retries = RetryConfig.maxRetries) {
    let lastError = null;
    let lastResponse = null;
    
    for (let i = 0; i <= retries; i++) {
        try {
            const res = method === 'POST' 
                ? http.post(url, payload, params)
                : http.get(url, params);
            
            lastResponse = res;
            
            if (RetryConfig.retryableStatusCodes.includes(res.status) && i < retries) {
                const backoffTime = RetryConfig.backoffBase * (i + 1);
                console.warn(`[Retry ${i + 1}/${retries}] 状态码 ${res.status}, 等待 ${backoffTime}s 后重试`);
                sleep(backoffTime);
                continue;
            }
            
            return res;
        } catch (e) {
            lastError = e;
            if (i < retries) {
                const backoffTime = RetryConfig.backoffBase * (i + 1);
                console.warn(`[Retry ${i + 1}/${retries}] 请求异常: ${e.message}, 等待 ${backoffTime}s 后重试`);
                sleep(backoffTime);
            }
        }
    }
    
    if (lastError) {
        console.error(`请求失败（已重试 ${retries} 次）: ${lastError.message}`);
        throw lastError;
    }
    
    return lastResponse;
}

export function createErrorResult(errorType, details = {}) {
    return {
        success: false,
        error: errorType,
        ...details,
        timestamp: new Date().toISOString(),
    };
}

export function createSuccessResult(data = {}) {
    return {
        success: true,
        ...data,
        timestamp: new Date().toISOString(),
    };
}

export function logPerformance(operation, duration, success) {
    const status = success ? '成功' : '失败';
    console.log(`[${operation}] ${status}, 耗时: ${duration.toFixed(2)}ms`);
}
