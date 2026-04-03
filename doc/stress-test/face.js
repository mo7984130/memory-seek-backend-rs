import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from './config.js';
import {getAuthHeaders, login} from './auth.js';

// ==================== 读操作 ====================

/**
 * 获取人物分页列表
 */
export function getPersonPage(headers, cursor = null, size = 20) {
    const url = `${config.baseUrl}/photo/face/person`;
    const params = { cursor, size };

    const res = http.get(url, { headers, params, timeout: config.timeout });

    check(res, {
        '人物列表状态码为 200': (r) => r.status === 200,
        '人物列表响应时间<400ms': (r) => r.timings.duration < 400,
    });

    return res;
}

/**
 * 获取人物详情
 */
export function getPersonDetail(personId, headers) {
    const url = `${config.baseUrl}/photo/face/person/${personId}`;

    const res = http.get(url, { headers, timeout: config.timeout });

    check(res, {
        '人物详情状态码为 200': (r) => r.status === 200,
        '人物详情响应时间<300ms': (r) => r.timings.duration < 300,
    });

    return res;
}

/**
 * 搜索人物
 */
export function searchPerson(headers, keyword, cursor = null, size = 20) {
    const url = `${config.baseUrl}/photo/face/person/search`;
    const params = { keyword, cursor, size };

    const res = http.get(url, { headers, params, timeout: config.timeout });

    check(res, {
        '搜索人物状态码为 200': (r) => r.status === 200,
        '搜索人物响应时间<300ms': (r) => r.timings.duration < 300,
    });

    return res;
}

/**
 * 获取人物的照片
 */
export function getPersonPhotos(personId, headers, cursor = null, size = 20) {
    const url = `${config.baseUrl}/photo/face/person/${personId}/photo`;
    const params = { cursor, size };

    const res = http.get(url, { headers, params, timeout: config.timeout });

    check(res, {
        '人物照片状态码为 200': (r) => r.status === 200,
        '人物照片响应时间<400ms': (r) => r.timings.duration < 400,
    });

    return res;
}

/**
 * 获取所有人脸列表（用于搜索）
 */
export function getAllPersons(headers) {
    const url = `${config.baseUrl}/photo/face/person/all`;

    const res = http.get(url, { headers, timeout: config.timeout });

    check(res, {
        '所有人脸状态码为 200': (r) => r.status === 200,
        '所有人脸响应时间<300ms': (r) => r.timings.duration < 300,
    });

    return res;
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

    // 2. 获取人物列表（读）
    const personPageRes = getPersonPage(headers);
    let personId = null;

    // 尝试从响应中获取人物 ID
    if (personPageRes.status === 200) {
        try {
            const body = JSON.parse(personPageRes.body);
            const persons = body.data?.records || [];
            if (persons.length > 0) {
                const randomIndex = Math.floor(Math.random() * persons.length);
                personId = persons[randomIndex].id;
                console.log(`选择人物：${personId}`);
            }
        } catch (e) {
            console.log('解析人物列表失败:', e.message);
        }
    }
    sleep(0.3);

    // 3. 如果有人物数据，获取人物详情（读）
    if (personId) {
        const detailRes = getPersonDetail(personId, headers);
        if (detailRes.status === 200) {
            try {
                const body = JSON.parse(detailRes.body);
                console.log(`获取人物详情成功：${body.data?.name || personId}`);
            } catch {}
        }
        sleep(0.3);

        // 4. 获取人物的照片（读）
        const photoRes = getPersonPhotos(personId, headers);
        if (photoRes.status === 200) {
            try {
                const body = JSON.parse(photoRes.body);
                console.log(`获取人物照片成功：${body.data?.records?.length || 0} 张`);
            } catch {}
        }
        sleep(0.3);
    }

    // 5. 搜索人物测试（读）
    // 先获取所有人脸列表，找到可用的关键词
    const allPersonsRes = getAllPersons(headers);
    let searchKeyword = 'test';

    if (allPersonsRes.status === 200) {
        try {
            const body = JSON.parse(allPersonsRes.body);
            const persons = body.data || [];
            if (persons.length > 0) {
                // 随机选择一个人名作为搜索关键词
                const randomIndex = Math.floor(Math.random() * persons.length);
                searchKeyword = persons[randomIndex].name || persons[randomIndex].id;
                console.log(`使用关键词搜索：${searchKeyword}`);
            }
        } catch (e) {
            console.log('获取所有人脸失败，使用默认关键词');
        }
    }
    sleep(0.3);

    // 执行搜索
    const searchRes = searchPerson(headers, searchKeyword);
    if (searchRes.status === 200) {
        try {
            const body = JSON.parse(searchRes.body);
            console.log(`搜索结果：${body.data?.records?.length || 0} 条`);
        } catch {}
    }
    sleep(0.5);

    // 6. 测试分页浏览多个人物
    for (let i = 0; i < 2; i++) {
        const pageRes = getPersonPage(headers, null, 10);
        if (pageRes.status === 200) {
            try {
                const body = JSON.parse(pageRes.body);
                const cursor = body.data?.nextCursor;
                if (cursor) {
                    // 使用游标继续分页
                    getPersonPage(headers, cursor, 10);
                }
            } catch {}
        }
        sleep(0.5);
    }

    sleep(1);
}
