import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from './config.js';
import {getAuthHeaders, login} from './auth.js';

export function getPhotoCursor(cursor = null, size = 20) {
    const url = `${config.baseUrl}/photo/photo/cursor`;
    const params = {
        cursor,
        size,
        direction: 'next',
    };

    const res = http.get(url, { params });

    check(res, {
        '照片瀑布流状态码为200': (r) => r.status === 200,
        '照片瀑布流响应时间<400ms': (r) => r.timings.duration < 400,
        '照片瀑布流返回数据': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.code === 200 && Array.isArray(body.data?.records);
            } catch {
                return false;
            }
        },
    });

    return res;
}

export function getPhotoByTime(time, size = 20) {
    const url = `${config.baseUrl}/photo/photo/by-time`;
    const params = { time, size };

    const res = http.get(url, { params });

    check(res, {
        '按时间获取照片状态码为200': (r) => r.status === 200,
        '按时间获取照片响应时间<400ms': (r) => r.timings.duration < 400,
    });

    return res;
}

export function getPhotoTimelineStat(headers) {
    const url = `${config.baseUrl}/photo/photo-timeline-stat`;

    const res = http.get(url, { headers, timeout: config.timeout });

    check(res, {
        '时间线统计状态码为200': (r) => r.status === 200,
        '时间线统计响应时间<300ms': (r) => r.timings.duration < 300,
    });

    return res;
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

    getPhotoTimelineStat(headers);
    sleep(0.5);

    let cursor = null;
    for (let i = 0; i < 3; i++) {
        const res = getPhotoCursor(cursor, 20);

        if (res.status === 200) {
            try {
                const body = JSON.parse(res.body);
                cursor = body.data?.nextCursor;
                if (!body.data?.hasMore) break;
            } catch {
                break;
            }
        }

        sleep(1);
    }

    sleep(2);
}
