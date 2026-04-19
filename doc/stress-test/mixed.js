import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from './common/config.js';
import {getAuthHeaders, login} from './common/auth.js';

export const options = {
    stages: [
        { duration: '1m', target: 10 },
        { duration: '2m', target: 25 },
        { duration: '3m', target: 50 },
        { duration: '5m', target: 50 },
        { duration: '1m', target: 0 },
    ],
    thresholds: {
        http_req_duration: ['p(95)<500'],
        http_req_failed: ['rate<0.01'],
    },
};

function getPhotoCursor(headers, cursor = null, size = 20) {
    const url = `${config.baseUrl}/photo/photo/cursor`;
    const params = { cursor, size, direction: 'next' };
    return http.get(url, { headers, params, timeout: config.timeout });
}

function getPhotoTimelineStat(headers) {
    const url = `${config.baseUrl}/photo/photo-timeline-stat`;
    return http.get(url, { headers, timeout: config.timeout });
}

function getCollectionList(headers) {
    const url = `${config.baseUrl}/photo/collection`;
    return http.get(url, { headers, timeout: config.timeout });
}

function getCollectionPhotos(collectionId, headers, cursor = null) {
    const url = `${config.baseUrl}/photo/collection/${collectionId}/photos`;
    const params = { cursor, size: 20 };
    return http.get(url, { headers, params, timeout: config.timeout });
}

function getPersonPage(headers, cursor = null) {
    const url = `${config.baseUrl}/photo/face/person`;
    const params = { cursor, size: 20 };
    return http.get(url, { headers, params, timeout: config.timeout });
}

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

    const timelineRes = getPhotoTimelineStat(headers);
    check(timelineRes, {
        '时间线统计成功': (r) => r.status === 200,
    });
    sleep(0.3);

    const photoRes = getPhotoCursor(headers);
    check(photoRes, {
        '照片瀑布流成功': (r) => r.status === 200,
        '照片瀑布流响应时间<400ms': (r) => r.timings.duration < 400,
    });
    sleep(0.5);

    if (photoRes.status === 200) {
        try {
            const body = JSON.parse(photoRes.body);
            if (body.data?.hasMore && body.data?.nextCursor) {
                const nextRes = getPhotoCursor(headers, body.data.nextCursor);
                check(nextRes, { '翻页加载成功': (r) => r.status === 200 });
                sleep(0.5);
            }
        } catch {
        }
    }

    const collectionRes = getCollectionList(headers);
    check(collectionRes, {
        '收藏夹列表成功': (r) => r.status === 200,
        '收藏夹列表响应时间<300ms': (r) => r.timings.duration < 300,
    });
    sleep(0.3);

    if (collectionRes.status === 200) {
        try {
            const body = JSON.parse(collectionRes.body);
            const collections = body.data || [];
            if (collections.length > 0) {
                const randomIdx = Math.floor(Math.random() * collections.length);
                const detailRes = getCollectionPhotos(collections[randomIdx].id, headers);
                check(detailRes, { '收藏夹详情成功': (r) => r.status === 200 });
            }
        } catch {
        }
    }
    sleep(0.3);

    const personRes = getPersonPage(headers);
    check(personRes, {
        '人物列表成功': (r) => r.status === 200,
    });

    sleep(Math.random() * 2 + 1);
}
