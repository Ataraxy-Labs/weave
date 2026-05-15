"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    DEFAULT_MAX_POSTPONED_STATE_SIZE: null,
    parseMaxPostponedStateSize: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    DEFAULT_MAX_POSTPONED_STATE_SIZE: function() {
        return DEFAULT_MAX_POSTPONED_STATE_SIZE;
    },
    parseMaxPostponedStateSize: function() {
        return parseMaxPostponedStateSize;
    }
});
const DEFAULT_MAX_POSTPONED_STATE_SIZE = '100 MB';
function parseSizeLimit(size) {
    const bytes = require('next/dist/compiled/bytes').parse(size);
    if (bytes === null || isNaN(bytes) || bytes < 1) {
        return undefined;
    }
    return bytes;
}
function parseMaxPostponedStateSize(size) {
    return parseSizeLimit(size ?? DEFAULT_MAX_POSTPONED_STATE_SIZE);
}

//# sourceMappingURL=size-limit.js.map