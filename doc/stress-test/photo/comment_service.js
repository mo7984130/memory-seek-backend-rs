import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from '../common/config.js';
import {getAuthHeaders, login} from '../common/auth.js';

// ==================== 读操作 ====================

/**
 * 获取照片的评论列表
 */
export function getCommentList(photoId, headers, cursor = null, limit = 20) {
    const url = `${config.baseUrl}/photo/comment/${photoId}`;
    const params = { cursor, limit };

    const res = http.get(url, { headers, params, timeout: config.timeout });

    check(res, {
        '评论列表状态码为 200': (r) => r.status === 200,
        '评论列表响应时间<300ms': (r) => r.timings.duration < 300,
    });

    return res;
}

// ==================== 写操作 ====================

/**
 * 发布评论
 */
export function publishComment(photoId, content, headers) {
    const url = `${config.baseUrl}/photo/comment/${photoId}`;
    const payload = JSON.stringify({ content });

    const res = http.post(url, payload, { headers, timeout: config.timeout });

    check(res, {
        '发布评论状态码为 200': (r) => r.status === 200,
        '发布评论响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

/**
 * 删除评论
 */
export function deleteComment(commentId, headers) {
    const url = `${config.baseUrl}/photo/comment/${commentId}`;

    const res = http.del(url, null, { headers, timeout: config.timeout });

    check(res, {
        '删除评论状态码为 200': (r) => r.status === 200,
        '删除评论响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

/**
 * 点赞/取消点赞评论
 */
export function toggleLikeComment(commentId, headers) {
    const url = `${config.baseUrl}/photo/comment/${commentId}/like/toggle`;
    const payload = JSON.stringify({});

    const res = http.post(url, payload, { headers, timeout: config.timeout });

    check(res, {
        '点赞评论状态码为 200': (r) => r.status === 200,
        '点赞评论响应时间<300ms': (r) => r.timings.duration < 300,
    });

    return res;
}

// ==================== 辅助函数 ====================

/**
 * 获取照片列表
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
        { duration: '1m', target: 20 },
        { duration: '2m', target: 20 },
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

    // 2. 获取照片列表（读）
    const photos = getPhotos(headers);

    if (photos.length === 0) {
        console.log('没有可用的照片数据');
        sleep(1);
        return;
    }

    // 随机选择一张照片进行评论测试
    const randomPhotoIndex = Math.floor(Math.random() * photos.length);
    const photoId = photos[randomPhotoIndex].id;
    console.log(`选择照片进行评论测试：${photoId}`);

    // 3. 获取照片的评论列表（读）
    const commentListRes = getCommentList(photoId, headers);
    sleep(0.3);

    // 4. 发布评论（写）
    const timestamp = Date.now();
    const commentContent = `压测测试评论-${timestamp}`;
    const publishRes = publishComment(photoId, commentContent, headers);
    let commentId = null;

    if (publishRes.status === 200) {
        try {
            const body = JSON.parse(publishRes.body);
            if (body.code === 200 && body.data) {
                commentId = body.data?.id || body.data;
                console.log(`发布评论成功：${commentId}`);
            }
        } catch (e) {
            console.log('解析发布评论响应失败:', e.message);
        }
    }
    sleep(0.3);

    // 5. 再次获取评论列表验证发布成功（读）
    getCommentList(photoId, headers);
    sleep(0.3);

    // 6. 点赞评论（写）
    if (commentId) {
        const likeRes = toggleLikeComment(commentId, headers);
        if (likeRes.status === 200) {
            console.log(`点赞评论成功：${commentId}`);
        }
        sleep(0.3);

        // 7. 再次点赞（取消点赞）
        toggleLikeComment(commentId, headers);
        sleep(0.3);
    }

    // 8. 删除评论（写）
    if (commentId) {
        const deleteRes = deleteComment(commentId, headers);
        if (deleteRes.status === 200) {
            console.log(`删除评论成功：${commentId}`);
        }
        sleep(0.3);
    }

    sleep(1);
}
