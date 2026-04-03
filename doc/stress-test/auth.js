import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from './config.js';

export function login(account, password) {
    const url = `${config.baseUrl}/auth/login`;
    const payload = JSON.stringify({ account, password });
    const params = {
        headers: { 'Content-Type': 'application/json' },
        timeout: config.timeout,
    };

    const res = http.post(url, payload, params);

    const success = check(res, {
        '登录状态码为200': (r) => r.status === 200,
        '登录响应时间<500ms': (r) => r.timings.duration < 500,
        '登录返回accessToken': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.code === 200 && body.data?.accessToken !== undefined;
            } catch {
                return false;
            }
        },
    });

    if (success) {
        try {
            const body = JSON.parse(res.body);
            return {
                success: true,
                userId: body.data.id,
                accessToken: body.data.accessToken,
                refreshToken: body.data.refreshToken,
            };
        } catch {
            return { success: false };
        }
    }
    return { success: false };
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

    const res = http.get(url, params);

    check(res, {
        '刷新Token状态码为200': (r) => r.status === 200,
        '刷新Token响应时间<300ms': (r) => r.timings.duration < 300,
    });

    if (res.status === 200) {
        try {
            const body = JSON.parse(res.body);
            if (body.code === 200) {
                return body.data.accessToken;
            }
        } catch {
            return null;
        }
    }
    return null;
}

export function getAuthHeaders(userId, accessToken) {
    return {
        'Content-Type': 'application/json',
        'Authorization': `${userId} ${accessToken}`,
    };
}

export const options = {
    stages: [
        { duration: '30s', target: 10 },
        { duration: '1m', target: 50 },
        { duration: '2m', target: 50 },
        { duration: '30s', target: 0 },
    ],
    thresholds: config.thresholds,
};

export default function () {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    const result = login(user.account, user.password);

    check(result, {
        '登录成功': (r) => r.success === true,
    });

    sleep(1);
}
