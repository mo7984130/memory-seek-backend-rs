// tests/load/scenarios/photo/photo.js
// 照片服务压测场景（photo_service + timeline_stat_service）

import encoding from "k6/encoding";
import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
    buildLoadOptions,
    setupPreLogin,
    sessionFromData,
} from "../../helpers/common.js";

export { printSummary as handleSummary };

import { setSession, initSession, maybeRefreshSession } from "../../helpers/session.js";
import {
    uploadPhoto,
    listPhotos,
    getTimelineStats,
} from "../../helpers/domains/photo/photo.js";

// 直接嵌入 base64 JPEG，b64decode 返回 ArrayBuffer，绕过 open() 的 string 编码问题
const testImageB64 = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAUDBAQEAwUEBAQFBQUGBwwIBwcHBw8LCwkMEQ8SEhEPERETFhwXExQaFRERGCEYGh0dHx8fExciJCIeJBweHx7/2wBDAQUFBQcGBw4ICA4eFBEUHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh7/wAARCADIAMgDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDzGiiivlD7gKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooA//Z";
const baseImage = encoding.b64decode(testImageB64);

// 生成唯一图片（在 JPEG 结束标记 FFD9 后追加随机字节，改变 MD5 但不影响解码）
function getUniqueImage() {
    const extra = 16;
    const buf = new ArrayBuffer(baseImage.byteLength + extra);
    const view = new Uint8Array(buf);
    view.set(new Uint8Array(baseImage));
    for (let i = 0; i < extra; i++) {
        view[baseImage.byteLength + i] = Math.floor(Math.random() * 256);
    }
    return buf;
}

// ── 独立运行时的 options ──

const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);

export const options = buildLoadOptions({
    targetRps: 30,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 2000,
});

// 上传是显式 S3 场景；预登录不计入压测窗口，且不会在每轮迭代登出。
export function setup() {
    return setupPreLogin(getPhotoUserCredentials, PRE_ALLOCATED_VUS);
}

// ── 核心逻辑（独立运行和被导入时共用） ──

function runPhotoFlow(data) {
    const session = sessionFromData(data, __VU);
    if (session) {
        setSession(session);
    } else {
        const { account, password } = getPhotoUserCredentials(__VU);
        if (!initSession(account, password)) return;
    }
    maybeRefreshSession();

    // 1. 上传照片
    let result = uploadPhoto(getUniqueImage());
    recordResult("upload_photo", result);
    if (!result.success) {
        console.error("Upload failed");
        return;
    }

    // 2. 查询照片列表
    result = listPhotos(20);
    recordResult("list_photos", result);

    // 3. 查询时间线统计
    result = getTimelineStats();
    recordResult("timeline_stats", result);

    // 4. 再次查询照片列表（模拟用户浏览）
    for (let i = 0; i < 20; i++) {
        result = listPhotos(20);
        recordResult("list_photos", result);
    }

}

// ── 独立运行入口 ──

export default function (data) {
    runPhotoFlow(data);
}

// ── 被统一入口调用的 exec 函数 ──

export function photoExec(data) {
    runPhotoFlow(data);
}
