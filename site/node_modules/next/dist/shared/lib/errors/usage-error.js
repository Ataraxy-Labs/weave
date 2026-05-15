"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "UsageError", {
    enumerable: true,
    get: function() {
        return UsageError;
    }
});
class UsageError extends Error {
    constructor(message, docUrl){
        super(`${message}\n\nLearn more: ${docUrl}`);
        this.name = 'UsageError';
        // This error is meant to interrupt the server start/build process
        // but the stack trace isn't meaningful, as it points to internal code.
        this.stack = undefined;
    }
}

//# sourceMappingURL=usage-error.js.map