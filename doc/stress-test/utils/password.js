import { sleep } from 'k6';

export function generateRandomPassword() {
    const timestamp = Date.now();
    const random = Math.random().toString(36).substring(2, 9);
    return `pwd_${timestamp}_${random}`;
}

export function randomSleep(min = 0.5, max = 2.5) {
    const sleepTime = Math.random() * (max - min) + min;
    sleep(sleepTime);
}

export function normalSleep(mean = 1.5, stdDev = 0.5) {
    const u1 = Math.random();
    const u2 = Math.random();
    const z = Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
    const sleepTime = Math.max(0.1, mean + z * stdDev);
    sleep(sleepTime);
}
