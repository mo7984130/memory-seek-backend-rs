import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from './config.js';
import {getAuthHeaders, login} from './auth.js';
import {
    addPhotoToCollection,
    createCollection,
    deleteCollection,
    getCollectionList,
    getCollectionPhotos,
    removePhotoFromCollection
} from './collections.js';
import {deleteComment, getCommentList, publishComment, toggleLikeComment} from './comments.js';
import {getPersonDetail, getPersonPage, getPersonPhotos, searchPerson} from './face.js';

export const options = {
    scenarios: {
        // 场景 1: 登录测试
        login_test: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '30s', target: 10 },
                { duration: '1m', target: 50 },
                { duration: '2m', target: 50 },
                { duration: '30s', target: 0 },
            ],
            gracefulRampDown: '30s',
            exec: 'loginTest',
        },
        // 场景 2: 照片浏览测试
        photo_browse_test: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '30s', target: 10 },
                { duration: '2m', target: 50 },
                { duration: '3m', target: 50 },
                { duration: '30s', target: 0 },
            ],
            gracefulRampDown: '30s',
            exec: 'photoBrowseTest',
            startTime: '4m',
        },
        // 场景 3: 收藏夹管理测试
        collection_test: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '30s', target: 10 },
                { duration: '1m', target: 20 },
                { duration: '2m', target: 20 },
                { duration: '30s', target: 0 },
            ],
            gracefulRampDown: '30s',
            exec: 'collectionTest',
            startTime: '10m',
        },
        // 场景 4: 评论系统测试
        comment_test: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '30s', target: 10 },
                { duration: '1m', target: 15 },
                { duration: '2m', target: 15 },
                { duration: '30s', target: 0 },
            ],
            gracefulRampDown: '30s',
            exec: 'commentTest',
            startTime: '13m',
        },
        // 场景 5: 人物浏览测试
        face_test: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '30s', target: 10 },
                { duration: '1m', target: 15 },
                { duration: '2m', target: 15 },
                { duration: '30s', target: 0 },
            ],
            gracefulRampDown: '30s',
            exec: 'faceTest',
            startTime: '16m',
        },
        // 场景 6: 混合场景测试（完整用户旅程）
        mixed_test: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '1m', target: 25 },
                { duration: '3m', target: 40 },
                { duration: '4m', target: 40 },
                { duration: '1m', target: 0 },
            ],
            gracefulRampDown: '30s',
            exec: 'mixedTest',
            startTime: '20m',
        },
    },
    thresholds: {
        http_req_duration: ['p(95)<500'],
        http_req_failed: ['rate<0.01'],
    },
};

// ==================== 场景 1: 登录测试 ====================

export function loginTest() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    const result = login(user.account, user.password);
    check(result, { '登录测试成功': (r) => r.success === true });

    sleep(1);
}

// ==================== 场景 2: 照片浏览测试 ====================

export function photoBrowseTest() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    const authResult = login(user.account, user.password);
    if (!authResult.success) {
        sleep(1);
        return;
    }

    const headers = getAuthHeaders(authResult.userId, authResult.accessToken);

    // 获取时间线统计
    const timelineUrl = `${config.baseUrl}/photo/photo-timeline-stat`;
    const timelineRes = http.get(timelineUrl, { headers, timeout: config.timeout });
    check(timelineRes, { '时间线统计成功': (r) => r.status === 200 });

    sleep(0.5);

    // 分页加载照片
    const cursorUrl = `${config.baseUrl}/photo/photo/cursor`;
    let cursor = null;
    for (let i = 0; i < 3; i++) {
        const params = { cursor, size: 20, direction: 'next' };
        const res = http.get(cursorUrl, { headers, params, timeout: config.timeout });

        check(res, {
            '照片瀑布流成功': (r) => r.status === 200,
            '照片瀑布流响应时间<400ms': (r) => r.timings.duration < 400,
        });

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

// ==================== 场景 3: 收藏夹管理测试 ====================

export function collectionTest() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    const authResult = login(user.account, user.password);
    if (!authResult.success) {
        sleep(1);
        return;
    }

    const headers = getAuthHeaders(authResult.userId, authResult.accessToken);

    // 1. 获取收藏夹列表（读）
    const listRes = getCollectionList(headers);
    sleep(0.3);

    // 2. 浏览现有收藏夹（读）
    if (listRes.status === 200) {
        try {
            const body = JSON.parse(listRes.body);
            const collections = body.data || [];

            if (collections.length > 0) {
                const randomIndex = Math.floor(Math.random() * collections.length);
                const collectionId = collections[randomIndex].id;
                getCollectionPhotos(collectionId, headers);
                sleep(0.5);
            }
        } catch {
            console.log('解析收藏夹列表失败');
        }
    }

    // 3. 创建收藏夹（写）
    const timestamp = Date.now();
    const createRes = createCollection(headers, `测试收藏夹-${timestamp}`, '压测测试用');
    let collectionId = null;

    if (createRes.status === 200) {
        try {
            const body = JSON.parse(createRes.body);
            if (body.code === 200 && body.data) {
                collectionId = body.data.id;
            }
        } catch {}
    }
    sleep(0.3);

    // 4. 获取照片列表
    const photoUrl = `${config.baseUrl}/photo/photo/cursor`;
    const photoRes = http.get(photoUrl, { headers, params: { size: 1 }, timeout: config.timeout });
    let photoId = null;

    if (photoRes.status === 200) {
        try {
            const body = JSON.parse(photoRes.body);
            if (body.data?.records && body.data.records.length > 0) {
                photoId = body.data.records[0].id;
            }
        } catch {}
    }
    sleep(0.3);

    // 5. 添加照片到收藏夹（写）
    if (collectionId && photoId) {
        addPhotoToCollection(collectionId, photoId, headers);
        sleep(0.3);

        // 6. 从收藏夹移除照片（写）
        removePhotoFromCollection(collectionId, photoId, headers);
        sleep(0.3);
    }

    // 7. 删除收藏夹（写）
    if (collectionId) {
        deleteCollection(collectionId, headers);
        sleep(0.3);
    }

    sleep(1);
}

// ==================== 场景 4: 评论系统测试 ====================

export function commentTest() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    const authResult = login(user.account, user.password);
    if (!authResult.success) {
        sleep(1);
        return;
    }

    const headers = getAuthHeaders(authResult.userId, authResult.accessToken);

    // 1. 获取照片列表
    const photoUrl = `${config.baseUrl}/photo/photo/cursor`;
    const photoRes = http.get(photoUrl, { headers, params: { size: 1 }, timeout: config.timeout });
    let photoId = null;

    if (photoRes.status === 200) {
        try {
            const body = JSON.parse(photoRes.body);
            if (body.data?.records && body.data.records.length > 0) {
                photoId = body.data.records[0].id;
            }
        } catch {}
    }

    if (!photoId) {
        console.log('没有可用的照片');
        sleep(1);
        return;
    }

    // 2. 获取评论列表（读）
    getCommentList(photoId, headers);
    sleep(0.3);

    // 3. 发布评论（写）
    const timestamp = Date.now();
    const publishRes = publishComment(photoId, `压测评论-${timestamp}`, headers);
    let commentId = null;

    if (publishRes.status === 200) {
        try {
            const body = JSON.parse(publishRes.body);
            if (body.code === 200 && body.data) {
                commentId = body.data?.id || body.data;
            }
        } catch {}
    }
    sleep(0.3);

    // 4. 点赞评论（写）
    if (commentId) {
        toggleLikeComment(commentId, headers);
        sleep(0.3);

        // 5. 取消点赞
        toggleLikeComment(commentId, headers);
        sleep(0.3);

        // 6. 删除评论（写）
        deleteComment(commentId, headers);
        sleep(0.3);
    }

    sleep(1);
}

// ==================== 场景 5: 人物浏览测试 ====================

export function faceTest() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    const authResult = login(user.account, user.password);
    if (!authResult.success) {
        sleep(1);
        return;
    }

    const headers = getAuthHeaders(authResult.userId, authResult.accessToken);

    // 1. 获取人物列表（读）
    const personPageRes = getPersonPage(headers);
    let personId = null;

    if (personPageRes.status === 200) {
        try {
            const body = JSON.parse(personPageRes.body);
            const persons = body.data?.records || [];
            if (persons.length > 0) {
                const randomIndex = Math.floor(Math.random() * persons.length);
                personId = persons[randomIndex].id;
            }
        } catch {}
    }
    sleep(0.3);

    // 2. 获取人物详情（读）
    if (personId) {
        getPersonDetail(personId, headers);
        sleep(0.3);

        // 3. 获取人物的照片（读）
        getPersonPhotos(personId, headers);
        sleep(0.3);
    }

    // 4. 搜索人物（读）
    const allPersonsUrl = `${config.baseUrl}/photo/face/person/all`;
    const allPersonsRes = http.get(allPersonsUrl, { headers, timeout: config.timeout });
    let searchKeyword = 'test';

    if (allPersonsRes.status === 200) {
        try {
            const body = JSON.parse(allPersonsRes.body);
            const persons = body.data || [];
            if (persons.length > 0) {
                const randomIndex = Math.floor(Math.random() * persons.length);
                searchKeyword = persons[randomIndex].name || persons[randomIndex].id;
            }
        } catch {}
    }

    searchPerson(headers, searchKeyword);
    sleep(0.5);

    // 5. 分页浏览
    getPersonPage(headers, null, 10);
    sleep(0.5);

    sleep(1);
}

// ==================== 场景 6: 混合场景测试 ====================

export function mixedTest() {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    // 1. 登录
    const authResult = login(user.account, user.password);
    if (!authResult.success) {
        sleep(1);
        return;
    }

    const headers = getAuthHeaders(authResult.userId, authResult.accessToken);

    // 2. 获取时间线统计
    const timelineRes = http.get(
        `${config.baseUrl}/photo/photo-timeline-stat`,
        { headers, timeout: config.timeout }
    );
    check(timelineRes, { '时间线统计成功': (r) => r.status === 200 });
    sleep(0.3);

    // 3. 浏览照片
    const photoRes = http.get(
        `${config.baseUrl}/photo/photo/cursor`,
        { headers, params: { size: 20 }, timeout: config.timeout }
    );
    check(photoRes, { '照片瀑布流成功': (r) => r.status === 200 });
    sleep(0.5);

    // 4. 获取收藏夹列表
    const collectionRes = http.get(
        `${config.baseUrl}/photo/collection`,
        { headers, timeout: config.timeout }
    );
    check(collectionRes, { '收藏夹列表成功': (r) => r.status === 200 });

    if (collectionRes.status === 200) {
        try {
            const body = JSON.parse(collectionRes.body);
            const collections = body.data || [];
            if (collections.length > 0) {
                const randomIdx = Math.floor(Math.random() * collections.length);
                http.get(
                    `${config.baseUrl}/photo/collection/${collections[randomIdx].id}/photos`,
                    { headers, params: { size: 20 }, timeout: config.timeout }
                );
            }
        } catch {}
    }
    sleep(0.3);

    // 5. 获取人物列表
    const personRes = http.get(
        `${config.baseUrl}/photo/face/person`,
        { headers, params: { size: 20 }, timeout: config.timeout }
    );
    check(personRes, { '人物列表成功': (r) => r.status === 200 });
    sleep(0.3);

    // 6. 获取照片评论
    if (photoRes.status === 200) {
        try {
            const body = JSON.parse(photoRes.body);
            if (body.data?.records && body.data.records.length > 0) {
                const photoId = body.data.records[0].id;
                getCommentList(photoId, headers);
            }
        } catch {}
    }
    sleep(0.3);

    // 7. 随机行为：创建收藏夹或发布评论
    const randomAction = Math.random();
    if (randomAction < 0.3) {
        // 30% 概率创建收藏夹
        const timestamp = Date.now();
        const createRes = createCollection(headers, `测试-${timestamp}`, '');
        if (createRes.status === 200) {
            try {
                const body = JSON.parse(createRes.body);
                if (body.code === 200 && body.data) {
                    const collectionId = body.data.id;
                    sleep(0.5);
                    deleteCollection(collectionId, headers);
                }
            } catch {}
        }
    } else if (randomAction < 0.5) {
        // 20% 概率发布评论
        if (photoRes.status === 200) {
            try {
                const body = JSON.parse(photoRes.body);
                if (body.data?.records && body.data.records.length > 0) {
                    const photoId = body.data.records[0].id;
                    const publishRes = publishComment(photoId, '测试评论', headers);
                    if (publishRes.status === 200) {
                        try {
                            const commentBody = JSON.parse(publishRes.body);
                            const commentId = commentBody.data?.id || commentBody.data;
                            if (commentId) {
                                sleep(0.3);
                                deleteComment(commentId, headers);
                            }
                        } catch {}
                    }
                }
            } catch {}
        }
    }

    sleep(Math.random() * 2 + 1);
}

// ==================== 报告生成 ====================

export function handleSummary(data) {
    return {
        stdout: textSummary(data, { indent: ' ', enableColors: true }),
        'stress-test-report.json': JSON.stringify(data, null, 2),
    };
}

function textSummary(data, options) {
    const indent = options?.indent || '  ';
    const colors = options?.enableColors || false;

    let summary = '\n========== 压力测试报告 ==========\n\n';

    if (data.metrics.http_req_duration) {
        const avg = data.metrics.http_req_duration.values.avg;
        const p95 = data.metrics.http_req_duration.values['p(95)'];
        summary += `响应时间:\n`;
        summary += `${indent}平均：${avg.toFixed(2)}ms\n`;
        summary += `${indent}P95: ${p95.toFixed(2)}ms\n\n`;
    }

    if (data.metrics.http_req_failed) {
        const failRate = data.metrics.http_req_failed.values.rate * 100;
        summary += `错误率：${failRate.toFixed(2)}%\n\n`;
    }

    if (data.metrics.http_reqs) {
        const totalReqs = data.metrics.http_reqs.values.count;
        const rps = data.metrics.http_reqs.values.rate;
        summary += `请求统计:\n`;
        summary += `${indent}总请求数：${totalReqs}\n`;
        summary += `${indent}每秒请求数 (RPS): ${rps.toFixed(2)}\n\n`;
    }

    if (data.metrics.iterations) {
        const totalIters = data.metrics.iterations.values.count;
        summary += `迭代次数：${totalIters}\n\n`;
    }

    summary += '==================================\n';

    return summary;
}
