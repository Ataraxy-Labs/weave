export const RSC_HEADER = 'rsc';
export const ACTION_HEADER = 'next-action';
// TODO: Instead of sending the full router state, we only need to send the
// segment path. Saves bytes. Then we could also use this field for segment
// prefetches, which also need to specify a particular segment.
export const NEXT_ROUTER_STATE_TREE_HEADER = 'next-router-state-tree';
export const NEXT_ROUTER_PREFETCH_HEADER = 'next-router-prefetch';
// This contains the path to the segment being prefetched.
// TODO: If we change next-router-state-tree to be a segment path, we can use
// that instead. Then next-router-prefetch and next-router-segment-prefetch can
// be merged into a single enum.
export const NEXT_ROUTER_SEGMENT_PREFETCH_HEADER = 'next-router-segment-prefetch';
export const NEXT_HMR_REFRESH_HEADER = 'next-hmr-refresh';
export const NEXT_HMR_REFRESH_HASH_COOKIE = '__next_hmr_refresh_hash__';
export const NEXT_URL = 'next-url';
export const RSC_CONTENT_TYPE_HEADER = 'text/x-component';
export const FLIGHT_HEADERS = [
    RSC_HEADER,
    NEXT_ROUTER_STATE_TREE_HEADER,
    NEXT_ROUTER_PREFETCH_HEADER,
    NEXT_HMR_REFRESH_HEADER,
    NEXT_ROUTER_SEGMENT_PREFETCH_HEADER
];
export const NEXT_RSC_UNION_QUERY = '_rsc';
export const NEXT_ROUTER_STALE_TIME_HEADER = 'x-nextjs-stale-time';
export const NEXT_DID_POSTPONE_HEADER = 'x-nextjs-postponed';
export const NEXT_REWRITTEN_PATH_HEADER = 'x-nextjs-rewritten-path';
export const NEXT_REWRITTEN_QUERY_HEADER = 'x-nextjs-rewritten-query';
export const NEXT_IS_PRERENDER_HEADER = 'x-nextjs-prerender';
export const NEXT_ACTION_NOT_FOUND_HEADER = 'x-nextjs-action-not-found';
export const NEXT_REQUEST_ID_HEADER = 'x-nextjs-request-id';
export const NEXT_HTML_REQUEST_ID_HEADER = 'x-nextjs-html-request-id';
// TODO: Should this include nextjs in the name, like the others?
export const NEXT_ACTION_REVALIDATED_HEADER = 'x-action-revalidated';

//# sourceMappingURL=app-router-headers.js.map