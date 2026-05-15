"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "step", {
    enumerable: true,
    get: function() {
        return step;
    }
});
const _test = require("@playwright/test");
async function step(_testInfo, props, handler) {
    let result;
    let reportedError;
    try {
        await _test.test.step(props.title, async ()=>{
            result = await handler(({ error })=>{
                reportedError = error;
                if (reportedError) {
                    throw reportedError;
                }
            });
        });
    } catch (error) {
        if (error !== reportedError) {
            throw error;
        }
    }
    return result;
}

//# sourceMappingURL=step.js.map