"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "getMemoryCache", {
    enumerable: true,
    get: function() {
        return getMemoryCache;
    }
});
const _types = require("../../response-cache/types");
const _lrucache = require("../lru-cache");
let memoryCache;
function getMemoryCache(maxMemoryCacheSize) {
    if (!memoryCache) {
        memoryCache = new _lrucache.LRUCache(maxMemoryCacheSize, function length({ value }) {
            var _JSON_stringify;
            if (!value) {
                return 25;
            } else if (value.kind === _types.CachedRouteKind.REDIRECT) {
                return JSON.stringify(value.props).length;
            } else if (value.kind === _types.CachedRouteKind.IMAGE) {
                throw Object.defineProperty(new Error('invariant image should not be incremental-cache'), "__NEXT_ERROR_CODE", {
                    value: "E501",
                    enumerable: false,
                    configurable: true
                });
            } else if (value.kind === _types.CachedRouteKind.FETCH) {
                return JSON.stringify(value.data || '').length;
            } else if (value.kind === _types.CachedRouteKind.APP_ROUTE) {
                return value.body.length;
            }
            // rough estimate of size of cache value
            return value.html.length + (((_JSON_stringify = JSON.stringify(value.kind === _types.CachedRouteKind.APP_PAGE ? value.rscData : value.pageData)) == null ? void 0 : _JSON_stringify.length) || 0);
        });
    }
    return memoryCache;
}

//# sourceMappingURL=memory-cache.external.js.map