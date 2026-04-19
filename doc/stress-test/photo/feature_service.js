import http from 'k6/http';
import {check, sleep} from 'k6';
import {config} from '../common/config.js';
import {getAuthHeaders, login} from '../common/auth.js';

export function getPhotoFeatures(photoId, headers) {
    const url = `${config.baseUrl}/photo/feature/photo/${photoId}`;
    const params = {
        headers,
        timeout: config.timeout,
    };

    const res = http.get(url, params);

    check(res, {
        '获取照片人脸特征状态码为200': (r) => r.status === 200,
        '获取照片人脸特征响应时间<300ms': (r) => r.timings.duration < 300,
        '获取照片人脸特征返回数据': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.code === 200 && Array.isArray(body.data);
            } catch {
                return false;
            }
        },
    });

    return res;
}

export function deleteFeature(featureId, headers) {
    const url = `${config.baseUrl}/photo/feature/${featureId}`;
    const params = {
        headers,
        timeout: config.timeout,
    };

    const res = http.del(url, null, params);

    check(res, {
        '删除人脸特征状态码为200': (r) => r.status === 200,
        '删除人脸特征响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

export function changeFaceBelonging(featureId, personId, headers) {
    const url = `${config.baseUrl}/photo/feature/${featureId}/person`;
    const payload = JSON.stringify({ personId });
    const params = {
        headers,
        timeout: config.timeout,
    };

    const res = http.put(url, payload, params);

    check(res, {
        '更改人脸归属状态码为200': (r) => r.status === 200,
        '更改人脸归属响应时间<500ms': (r) => r.timings.duration < 500,
    });

    return res;
}

function getPhotos(headers, size = 20) {
    const url = `${config.baseUrl}/photo/photo/cursor`;
    const params = { size };

    const res = http.get(url, { headers, params, timeout: config.timeout });

    if (res.status === 200) {
        try {
            const body = JSON.parse(res.body);
            if (body.code === 200 && body.data?.records) {
                return body.data.records;
            }
        } catch {}
    }
    return [];
}

function getPersonPage(headers, cursor = null, size = 20) {
    const url = `${config.baseUrl}/photo/face/person`;
    const params = { cursor, size };

    const res = http.get(url, { headers, params, timeout: config.timeout });

    if (res.status === 200) {
        try {
            const body = JSON.parse(res.body);
            return body.data?.records || [];
        } catch {}
    }
    return [];
}

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

    const authResult = login(user.account, user.password);
    if (!authResult.success) {
        console.log(`登录失败: ${user.account}`);
        sleep(1);
        return;
    }

    const headers = getAuthHeaders(authResult.userId, authResult.accessToken);

    const photos = getPhotos(headers);
    if (photos.length === 0) {
        console.log('没有可用的照片数据');
        sleep(1);
        return;
    }

    const randomPhotoIndex = Math.floor(Math.random() * photos.length);
    const photoId = photos[randomPhotoIndex].id;
    console.log(`选择照片进行人脸特征测试: ${photoId}`);

    const featuresRes = getPhotoFeatures(photoId, headers);
    sleep(0.3);

    if (featuresRes.status === 200) {
        try {
            const body = JSON.parse(featuresRes.body);
            const features = body.data || [];

            if (features.length > 0) {
                const featureId = features[0].id;
                console.log(`找到人脸特征: ${featureId}`);

                const persons = getPersonPage(headers);
                if (persons.length > 0) {
                    const personId = persons[0].id;
                    console.log(`尝试更改人脸归属到人物: ${personId}`);

                    const changeRes = changeFaceBelonging(featureId, personId, headers);
                    if (changeRes.status === 200) {
                        console.log(`更改人脸归属成功`);
                    }
                    sleep(0.3);
                }
            }
        } catch (e) {
            console.log('解析人脸特征失败:', e.message);
        }
    }

    sleep(1);
}
