import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from './config.js';
import {getAuthHeaders, login} from './auth.js';

// ==================== 读操作 ====================

/**
 * 获取收藏夹列表
 */
export function getCollectionList(headers) {
    const url = `${config.baseUrl}/photo/collection`;
    const res = http.get(url, { headers, timeout: config.timeout });

    check(res, {
        '收藏夹列表状态码为 200': (r) => r.status === 200,
        '收藏夹列表响应时间<300ms': (r) => r.timings.duration < 300,
        '收藏夹列表返回数据': (r) => {
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

/**
 * 获取收藏夹内的照片
 */
export function getCollectionPhotos(collectionId, headers, cursor = null, size = 20) {
    const url = `${config.baseUrl}/photo/collection/${collectionId}/photos`;
    const params = { cursor, size };

    const res = http.get(url, { headers, params, timeout: config.timeout });

    check(res, {
        '收藏夹照片状态码为 200': (r) => r.status === 200,
        '收藏夹照片响应时间<400ms': (r) => r.timings.duration < 400,
    });

    return res;
}

// ==================== 写操作 ====================

/**
 * 创建收藏夹
 */
export function createCollection(headers, name, description = '') {
    const url = `${config.baseUrl}/photo/collection`;
    const payload = JSON.stringify({ name, description });

    const res = http.post(url, payload, { headers, timeout: config.timeout });

    check(res, {
        '创建收藏夹状态码为 200': (r) => r.status === 200,
        '创建收藏夹响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

/**
 * 编辑收藏夹信息
 */
export function updateCollection(collectionId, headers, name, description = '') {
    const url = `${config.baseUrl}/photo/collection/${collectionId}`;
    const payload = JSON.stringify({ name, description });

    const res = http.patch(url, payload, { headers, timeout: config.timeout });

    check(res, {
        '编辑收藏夹状态码为 200': (r) => r.status === 200,
        '编辑收藏夹响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

/**
 * 添加照片到收藏夹
 */
export function addPhotoToCollection(collectionId, photoId, headers) {
    const url = `${config.baseUrl}/photo/collection/${collectionId}/photos/${photoId}`;
    const payload = JSON.stringify({});

    const res = http.post(url, payload, { headers, timeout: config.timeout });

    check(res, {
        '添加照片状态码为 200': (r) => r.status === 200,
        '添加照片响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

/**
 * 从收藏夹移除照片
 */
export function removePhotoFromCollection(collectionId, photoId, headers) {
    const url = `${config.baseUrl}/photo/collection/${collectionId}/photos/${photoId}`;

    const res = http.del(url, null, { headers, timeout: config.timeout });

    check(res, {
        '移除照片状态码为 200': (r) => r.status === 200,
        '移除照片响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

/**
 * 删除收藏夹
 */
export function deleteCollection(collectionId, headers) {
    const url = `${config.baseUrl}/photo/collection/${collectionId}`;

    const res = http.del(url, null, { headers, timeout: config.timeout });

    check(res, {
        '删除收藏夹状态码为 200': (r) => r.status === 200,
        '删除收藏夹响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

// ==================== 辅助函数 ====================

/**
 * 获取照片列表（用于获取测试用的 photoId）
 */
function getPhotos(headers, size = 20) {
    const url = `${config.baseUrl}/photo/photo/cursor`;
    const params = { size };

    const res = http.get(url, { headers, params, timeout: config.timeout });

    if (res.status === 200) {
        try {
            const body = JSON.parse(res.body);
            if (body.code === 200 && body.data?.records) {
                return body.data.records;
            }
        } catch {}
    }
    return [];
}

// ==================== 测试场景 ====================

export const options = {
    stages: [
        { duration: '30s', target: 10 },
        { duration: '1m', target: 30 },
        { duration: '2m', target: 30 },
        { duration: '30s', target: 0 },
    ],
    thresholds: config.thresholds,
};

export default function () {
    const userIndex = (__VU - 1) % config.testUsers.length;
    const user = config.testUsers[userIndex];

    // 1. 登录
    const authResult = login(user.account, user.password);
    if (!authResult.success) {
        console.log(`登录失败：${user.account}`);
        sleep(1);
        return;
    }

    const headers = getAuthHeaders(authResult.userId, authResult.accessToken);

    // 2. 获取收藏夹列表（读）
    const listRes = getCollectionList(headers);
    sleep(0.3);

    // 3. 浏览现有收藏夹（读）
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

    // 4. 创建测试收藏夹（写）
    const timestamp = Date.now();
    const createRes = createCollection(headers, `测试收藏夹-${timestamp}`, '压测测试用');
    let collectionId = null;

    if (createRes.status === 200) {
        try {
            const body = JSON.parse(createRes.body);
            if (body.code === 200 && body.data) {
                collectionId = body.data.id;
                console.log(`创建收藏夹成功：${collectionId}`);
            }
        } catch (e) {
            console.log('解析创建收藏夹响应失败:', e.message);
        }
    }
    sleep(0.3);

    // 5. 获取照片列表
    const photos = getPhotos(headers);
    if (photos.length > 0 && collectionId) {
        const photoId = photos[0].id;

        // 6. 添加照片到收藏夹（写）
        const addRes = addPhotoToCollection(collectionId, photoId, headers);
        if (addRes.status === 200) {
            console.log(`添加照片到收藏夹成功：${collectionId}, ${photoId}`);
        }
        sleep(0.3);

        // 7. 从收藏夹移除照片（写）
        const removeRes = removePhotoFromCollection(collectionId, photoId, headers);
        if (removeRes.status === 200) {
            console.log(`从收藏夹移除照片成功：${collectionId}, ${photoId}`);
        }
        sleep(0.3);
    }

    // 8. 删除测试收藏夹（写）
    if (collectionId) {
        const deleteRes = deleteCollection(collectionId, headers);
        if (deleteRes.status === 200) {
            console.log(`删除收藏夹成功：${collectionId}`);
        }
        sleep(0.3);
    }

    sleep(1);
}
