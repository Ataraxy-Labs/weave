"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "flightRouterStateSchema", {
    enumerable: true,
    get: function() {
        return flightRouterStateSchema;
    }
});
const _superstruct = /*#__PURE__*/ _interop_require_default(require("next/dist/compiled/superstruct"));
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const dynamicParamTypesSchema = _superstruct.default.enums([
    'c',
    'ci(..)(..)',
    'ci(.)',
    'ci(..)',
    'ci(...)',
    'oc',
    'd',
    'di(..)(..)',
    'di(.)',
    'di(..)',
    'di(...)'
]);
const segmentSchema = _superstruct.default.union([
    _superstruct.default.string(),
    _superstruct.default.tuple([
        // Param name
        _superstruct.default.string(),
        // Param cache key (almost the same as the value, but arrays are
        // concatenated into strings)
        // TODO: We should change this to just be the value. Currently we convert
        // it back to a value when passing to useParams. It only needs to be
        // a string when converted to a a cache key, but that doesn't mean we
        // need to store it as that representation.
        _superstruct.default.string(),
        // Dynamic param type
        dynamicParamTypesSchema
    ])
]);
const flightRouterStateSchema = _superstruct.default.tuple([
    segmentSchema,
    _superstruct.default.record(_superstruct.default.string(), _superstruct.default.lazy(()=>flightRouterStateSchema)),
    _superstruct.default.optional(_superstruct.default.nullable(_superstruct.default.string())),
    _superstruct.default.optional(_superstruct.default.nullable(_superstruct.default.union([
        _superstruct.default.literal('refetch'),
        _superstruct.default.literal('refresh'),
        _superstruct.default.literal('inside-shared-layout'),
        _superstruct.default.literal('metadata-only')
    ]))),
    _superstruct.default.optional(_superstruct.default.boolean())
]);

//# sourceMappingURL=types.js.map