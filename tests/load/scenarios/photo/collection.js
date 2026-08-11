// tests/load/scenarios/photo/collection.js
// 收藏夹服务压测场景 — arrival-rate 负载模型, setup 预登录
//
// 完整流程迭代(create/list/update/delete 自洽, 数据不累积),
// 会话由 setup 预登录(login 不计入压测窗口), 全程复用。

import {
    getPhotoUserCredentials,
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
    createCollection,
    listCollections,
    updateCollection,
    deleteCollection,
} from "../../helpers/domains/photo/collection.js";

export { printSummary as handleSummary };

const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);

export const options = buildLoadOptions({
    targetRps: 60,
    maxRps: 150,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 2000,
});

// setup 预登录: login 不计入压测窗口
export function setup() {
    return setupPreLogin(getPhotoUserCredentials, PRE_ALLOCATED_VUS);
}

// ── 核心逻辑 ──

function runCollectionFlow(data) {
    const session = sessionFromData(data, __VU);
    if (session) {
        setSession(session);
    } else {
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

export default function (data) {
    runCollectionFlow(data);
}

// ── 被统一入口(photo.js)调用的 exec 函数 ──

export function collectionExec(data) {
    runCollectionFlow(data);
}
