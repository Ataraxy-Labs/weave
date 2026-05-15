export const ACTION_REFRESH = 'refresh';
export const ACTION_NAVIGATE = 'navigate';
export const ACTION_RESTORE = 'restore';
export const ACTION_SERVER_PATCH = 'server-patch';
export const ACTION_HMR_REFRESH = 'hmr-refresh';
export const ACTION_SERVER_ACTION = 'server-action';
/**
 * PrefetchKind defines the type of prefetching that should be done.
 * - `auto` - if the page is dynamic, prefetch the page data partially, if static prefetch the page data fully.
 * - `full` - prefetch the page data fully.
 */ export var PrefetchKind = /*#__PURE__*/ function(PrefetchKind) {
    PrefetchKind["AUTO"] = "auto";
    PrefetchKind["FULL"] = "full";
    return PrefetchKind;
}({});

//# sourceMappingURL=router-reducer-types.js.map