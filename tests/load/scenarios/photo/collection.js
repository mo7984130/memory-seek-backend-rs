// tests/load/scenarios/photo/collection.js
// 收藏夹服务压测场景 — arrival-rate 负载模型
//
// 完整流程迭代(create/list/update/delete 自洽, 数据不累积), 会话跨迭代复用。

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
import {
    createCollection,
    listCollections,
    updateCollection,
    deleteCollection,
} from "../../helpers/domains/photo/collection.js";

export { printSummary as handleSummary };

export const options = buildLoadOptions({
    targetRps: 30,
    maxRps: 150,
    preAllocatedVUs: 50,
    maxVUs: 200,
});

// ── 核心逻辑 ──

function runCollectionFlow() {
    if (!getSession()) {
        const { account, password } = getPhotoUserCredentials(__VU);
        initSession(account, password);
        return;
    }
    maybeRefreshSession();

    let result = createCollection(`Collection ${__VU} ${Date.now()}`, "LoadTest");
    recordResult("create_collection", result);
    if (!result.success) return;
    const collectionId = result.data.id;

    result = listCollections();
    recordResult("list_collections", result);

    result = updateCollection(collectionId, `Updated ${__VU}`, "Updated desc");
    recordResult("update_collection", result);

    result = deleteCollection(collectionId);
    recordResult("delete_collection", result);
}

// ── 独立运行入口 ──

export default function () {
    runCollectionFlow();
}

// ── 被统一入口(photo.js)调用的 exec 函数 ──

export function collectionExec() {
    runCollectionFlow();
}
