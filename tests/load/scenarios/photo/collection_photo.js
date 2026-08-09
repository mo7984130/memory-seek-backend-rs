// tests/load/scenarios/photo/collection_photo.js
// 收藏夹-照片关联服务压测场景 — arrival-rate 负载模型
//
// 完整流程迭代(建夹/加照片/查询/移除/删夹 自洽), 会话跨迭代复用。

import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
    buildLoadOptions,
} from "../../helpers/common.js";
import {
    initSession,
    getSession,
    maybeRefreshSession,
} from "../../helpers/session.js";
import { listPhotos } from "../../helpers/domains/photo/photo.js";
import { createCollection, deleteCollection } from "../../helpers/domains/photo/collection.js";
import {
    addPhotosToCollection,
    listCollectionPhotos,
    removePhotoFromCollection,
} from "../../helpers/domains/photo/collection_photo.js";

export { printSummary as handleSummary };

export const options = buildLoadOptions({
    targetRps: 20,
    maxRps: 100,
    preAllocatedVUs: 50,
    maxVUs: 200,
});

// ── 核心逻辑 ──

function runCollectionPhotoFlow() {
    if (!getSession()) {
        const { account, password } = getPhotoUserCredentials(__VU);
        initSession(account, password);
        return;
    }
    maybeRefreshSession();

    const photoListResult = listPhotos(1);
    if (!photoListResult.success || !photoListResult.data?.length) {
        return;
    }
    const photoId = photoListResult.data[0].id;

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

export default function () {
    runCollectionPhotoFlow();
}

// ── 被统一入口(photo.js)调用的 exec 函数 ──

export function collectionPhotoExec() {
    runCollectionPhotoFlow();
}
