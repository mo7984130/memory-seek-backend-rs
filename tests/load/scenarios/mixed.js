// tests/load/scenarios/mixed.js
// 混合场景压测 — 按权重随机选择操作，模拟真实用户行为

import { sleep } from "k6";
import { SharedArray } from "k6/data";
import { getPhotoUserCredentials, recordResult } from "../helpers/common.js";
import { initSession, refreshSession, logout } from "../helpers/session.js";
import { getMe } from "../helpers/user_ops.js";
import { uploadPhoto, listPhotos, getTimelineStats } from "../helpers/photo_ops.js";
import { createCollection, listCollections } from "../helpers/collection_ops.js";
import { createComment, listComments, likeComment } from "../helpers/comment_ops.js";

// 共享图片数据
const testImage = new SharedArray("test-image", function () {
    return [open("../fixtures/test.jpg", "b")];
});

// ── 操作定义 ──

const OPERATIONS = [
    { weight: 30, fn: () => listPhotos(20), label: "list_photos" },
    { weight: 20, fn: () => getTimelineStats(), label: "timeline_stats" },
    { weight: 15, fn: () => getMe(), label: "get_me" },
    { weight: 10, fn: () => listCollections(10), label: "list_collections" },
    { weight: 10, fn: () => listCommentsFromPool(), label: "list_comments" },
    { weight: 5, fn: () => createCommentFromPool(), label: "create_comment" },
    { weight: 5, fn: () => likeCommentFromPool(), label: "like_comment" },
    { weight: 3, fn: () => uploadPhoto(testImage[0]), label: "upload_photo" },
    { weight: 2, fn: () => createCollection(`Mixed ${__VU} ${Date.now()}`), label: "create_collection" },
];

// ── 辅助：从照片池中随机取一张照片的 ID ──

let _photoPool = null;

function getPhotoIdFromPool() {
    if (!_photoPool) {
        const result = listPhotos(50);
        if (result.success && result.data && result.data.length > 0) {
            _photoPool = result.data;
        } else {
            return null;
        }
    }
    const idx = Math.floor(Math.random() * _photoPool.length);
    return _photoPool[idx]?.id ?? null;
}

function listCommentsFromPool() {
    const photoId = getPhotoIdFromPool();
    if (!photoId) return { success: false, duration: 0 };
    return listComments(photoId);
}

function createCommentFromPool() {
    const photoId = getPhotoIdFromPool();
    if (!photoId) return { success: false, duration: 0 };
    return createComment(photoId, `Mixed comment VU${__VU} ${Date.now()}`);
}

function likeCommentFromPool() {
    const photoId = getPhotoIdFromPool();
    if (!photoId) return { success: false, duration: 0 };
    // 先获取一条评论来点赞
    const comments = listComments(photoId);
    if (comments.success && comments.data && comments.data.length > 0) {
        const commentId = comments.data[0].id;
        return likeComment(photoId, commentId);
    }
    return { success: false, duration: 0 };
}

// ── 权重随机选择 ──

function pickOperation() {
    const total = OPERATIONS.reduce((s, o) => s + o.weight, 0);
    let rand = Math.random() * total;
    for (const op of OPERATIONS) {
        rand -= op.weight;
        if (rand <= 0) return op;
    }
    return OPERATIONS[0];
}

// ── k6 配置 ──

export const options = {
    stages: [
        { duration: "1m", target: 30 },
        { duration: "3m", target: 30 },
        { duration: "1m", target: 60 },
        { duration: "3m", target: 60 },
        { duration: "1m", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<500"],
        http_req_failed: ["rate<0.02"],
        op_errors: ["rate<0.02"],
    },
};

// ── 主函数 ──

export default function () {
    const { account, password } = getPhotoUserCredentials(__VU);

    // 登录
    const session = initSession(account, password);
    if (!session) return;

    // 重置照片池（每个 VU 独立）
    _photoPool = null;

    // 执行 5~15 个随机操作
    const opCount = 5 + Math.floor(Math.random() * 11);
    for (let i = 0; i < opCount; i++) {
        const op = pickOperation();
        const result = op.fn();
        recordResult(op.label, result);

        // 随机思考时间 0.5~2s
        sleep(0.5 + Math.random() * 1.5);
    }

    // 30% 概率刷新 token
    if (Math.random() < 0.3) {
        refreshSession();
    }

    // 登出
    logout();

    sleep(0.5);
}
