// tests/load/scenarios/photo/photo_read.js
// 照片读取与时间线压测场景 — 不依赖对象存储，使用预置照片数据。

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
    listPhotos,
    getTimelineStats,
} from "../../helpers/domains/photo/photo.js";

export { printSummary as handleSummary };

const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);

export const options = buildLoadOptions({
    targetRps: 100,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 2000,
});

export function setup() {
    return setupPreLogin(getPhotoUserCredentials, PRE_ALLOCATED_VUS);
}

function runPhotoReadFlow(data) {
    const session = sessionFromData(data, __VU);
    if (session) {
        setSession(session);
    } else {
        const { account, password } = getPhotoUserCredentials(__VU);
        if (!initSession(account, password)) return;
    }
    maybeRefreshSession();

    recordResult("list_photos", listPhotos(20));
    recordResult("timeline_stats", getTimelineStats());
}

export default function (data) {
    runPhotoReadFlow(data);
}

export function photoReadExec(data) {
    runPhotoReadFlow(data);
}
