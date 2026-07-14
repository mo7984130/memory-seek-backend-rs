// tests/load/scenarios/photo/comment.js
// 评论服务压测场景（comment_controller）

import { sleep } from "k6";
import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
} from "../../helpers/common.js";

export { printSummary as handleSummary };

import { initSession } from "../../helpers/session.js";
import { listPhotos } from "../../helpers/domains/photo/photo.js";
import {
    createComment,
    listComments,
    deleteComment,
} from "../../helpers/domains/photo/comment.js";

// ── 独立运行时的 options ──

export const options = {
    stages: [
        { duration: "30s", target: 10 },
        { duration: "1m", target: 10 },
        { duration: "30s", target: 20 },
        { duration: "1m", target: 20 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<500"],
        http_req_failed: ["rate<0.01"],
    },
};

// ── 核心逻辑 ──

function runCommentFlow() {
    const { account, password } = getPhotoUserCredentials(__VU);
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 获取一张照片 ID
    const photoListResult = listPhotos(1);
    if (!photoListResult.success || !photoListResult.data?.length) {
        return;
    }
    const photoId = photoListResult.data[0].id;

    sleep(0.3);

    // 1. 发表评论
    let result = createComment(photoId, `Comment VU${__VU} ${Date.now()}`);
    recordResult("create_comment", result);
    if (!result.success) return;
    const commentId = result.data.id;

    sleep(0.3);

    // 2. 查询评论列表
    result = listComments(photoId);
    recordResult("list_comments", result);

    sleep(0.3);

    // 3. 删除评论
    result = deleteComment(photoId, commentId);
    recordResult("delete_comment", result);

    sleep(0.5);
}

// ── 独立运行入口 ──

export default function () {
    runCommentFlow();
}

// ── 被统一入口调用的 exec 函数 ──

export function commentExec() {
    runCommentFlow();
}
