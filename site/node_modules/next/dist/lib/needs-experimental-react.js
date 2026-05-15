"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "needsExperimentalReact", {
    enumerable: true,
    get: function() {
        return needsExperimentalReact;
    }
});
function needsExperimentalReact(config) {
    const { taint, transitionIndicator } = config.experimental || {};
    return Boolean(taint || transitionIndicator);
}

//# sourceMappingURL=needs-experimental-react.js.map