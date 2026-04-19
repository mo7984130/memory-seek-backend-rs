import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from '../common/config.js';
import {getAuthHeaders, login} from '../common/auth.js';

export function getTimelineStats(headers) {
    const url = `${config.baseUrl}/photo/timeline-stat`;
    const params = {
        headers,
        timeout: config.timeout,
    };

    const res = http.get(url, params);

    check(res, {
        '获取时间线统计状态码为200': (r) => r.status === 200,
        '获取时间线统计响应时间<300ms': (r) => r.timings.duration < 300,
        '获取时间线统计返回数据': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.code === 200 && Array.isArray(body.data);
            } catch {
                return false;
            }
        },
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

    const statsRes = getTimelineStats(headers);

    if (statsRes.status === 200) {
        try {
            const body = JSON.parse(statsRes.body);
            const stats = body.data || [];
            console.log(`获取时间线统计成功，共 ${stats.length} 个月份的数据`);
            
            if (stats.length > 0) {
                const totalPhotos = stats.reduce((sum, stat) => sum + stat.count, 0);
                console.log(`总照片数: ${totalPhotos}`);
            }
        } catch (e) {
            console.log('解析时间线统计失败:', e.message);
        }
    }

    sleep(1);
}
