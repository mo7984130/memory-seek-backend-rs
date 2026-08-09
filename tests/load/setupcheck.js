import { setupPreLogin, getPhotoUserCredentials } from "./helpers/common.js";

export function setup() {
    const s = setupPreLogin(getPhotoUserCredentials, 3);
    console.log("SETUP_RESULT " + JSON.stringify(s));
    return s;
}

export default function (data) {
    console.log("DATA_LEN " + (data?.length ?? "undef"));
}

export const options = {
    vus: 1,
    iterations: 1,
    setupTimeout: "120s",
};
