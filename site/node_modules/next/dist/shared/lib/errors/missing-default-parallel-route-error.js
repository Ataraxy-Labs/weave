"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "MissingDefaultParallelRouteError", {
    enumerable: true,
    get: function() {
        return MissingDefaultParallelRouteError;
    }
});
const _usageerror = require("./usage-error");
class MissingDefaultParallelRouteError extends _usageerror.UsageError {
    constructor(fullSegmentPath, slotName){
        super(`Missing required default.js file for parallel route at ${fullSegmentPath}\n` + `The parallel route slot "${slotName}" is missing a default.js file. When using parallel routes, each slot must have a default.js file to serve as a fallback.\n\n` + `Create a default.js file at: ${fullSegmentPath}/default.js`, 'https://nextjs.org/docs/messages/slot-missing-default');
        this.name = 'MissingDefaultParallelRouteError';
    }
}

//# sourceMappingURL=missing-default-parallel-route-error.js.map