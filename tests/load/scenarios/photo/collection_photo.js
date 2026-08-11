// tests/load/scenarios/photo/collection_photo.js
// 收藏夹-照片关联服务压测场景 — arrival-rate 负载模型
//
// 完整流程迭代(建夹/加照片/查询/移除/删夹 自洽), 会话跨迭代复用。

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
import { createCollection, deleteCollection } from "../../helpers/domains/photo/collection.js";
import {
    addPhotosToCollection,
    listCollectionPhotos,
    removePhotoFromCollection,
} from "../../helpers/domains/photo/collection_photo.js";

export { printSummary as handleSummary };

const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);

export const options = buildLoadOptions({
    targetRps: 40,
    maxRps: 100,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 2000,
});

// setup 预登录: login 不计入压测窗口
export function setup() {
    return setupPreLogin(getPhotoUserCredentials, PRE_ALLOCATED_VUS);
}

// ── 核心逻辑 ──

function runCollectionPhotoFlow(data) {
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

    const collResult = createCollection(`CP ${__VU} ${Date.now()}`, "LoadTest");
    if (!collResult.success) return;
    const collectionId = collResult.data.id;

    let result = addPhotosToCollection(collectionId, [photoId]);
    recordResult("add_photos_to_collection", result);

    result = listCollectionPhotos(collectionId);
    recordResult("list_collection_photos", result);

    result = removePhotoFromCollection(collectionId, photoId);
    recordResult("remove_photo_from_collection", result);

    deleteCollection(collectionId);
}

// ── 独立运行入口 ──

export default function (data) {
    runCollectionPhotoFlow(data);
}

// ── 被统一入口(photo.js)调用的 exec 函数 ──

export function collectionPhotoExec(data) {
    runCollectionPhotoFlow(data);
}
