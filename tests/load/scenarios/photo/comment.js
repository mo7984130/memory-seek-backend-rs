// tests/load/scenarios/photo/comment.js
// 评论服务压测场景 — arrival-rate 负载模型
//
// 完整流程迭代(发评论/查列表/删评论 自洽), 会话跨迭代复用。

import {
    getPhotoUserCredentials,
    pickPhotoUserPhotoId,
    recordResult,
    printSummary,
    buildLoadOptions,
    setupPreLogin,
    sessionFromData,
} from "../../helpers/common.js";
import {
    setSession,
    initSession,
    maybeRefreshSession,
} from "../../helpers/session.js";
import {
    createComment,
    listComments,
    deleteComment,
} from "../../helpers/domains/photo/comment.js";

export { printSummary as handleSummary };

const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);

export const options = buildLoadOptions({
    targetRps: 50,
    maxRps: 150,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 2000,
});

// setup 预登录: login 不计入压测窗口
export function setup() {
    return setupPreLogin(getPhotoUserCredentials, PRE_ALLOCATED_VUS);
}

// ── 核心逻辑 ──

function runCommentFlow(data) {
    const session = sessionFromData(data, __VU);
    if (session) {
        setSession(session);
    } else {
        const { account, password } = getPhotoUserCredentials(__VU);
        initSession(account, password);
        return;
    }
    maybeRefreshSession();

    // 按 VU 分散取照片, 避免所有请求命中同一张最新照片造成行锁热点
    const photoId = pickPhotoUserPhotoId(__VU);

    let result = createComment(photoId, `Comment VU${__VU} ${Date.now()}`);
    recordResult("create_comment", result);
    if (!result.success) return;
    const commentId = result.data.id;

    result = listComments(photoId);
    recordResult("list_comments", result);

    result = deleteComment(photoId, commentId);
    recordResult("delete_comment", result);
}

// ── 独立运行入口 ──

export default function (data) {
    runCommentFlow(data);
}

// ── 被统一入口(photo.js)调用的 exec 函数 ──

export function commentExec(data) {
    runCommentFlow(data);
}
