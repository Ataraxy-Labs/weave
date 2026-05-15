import { DEFAULT_SEGMENT_KEY, NOT_FOUND_SEGMENT_KEY } from '../../../shared/lib/segment';
import { matchSegment } from '../match-segments';
import { createHrefFromUrl } from './create-href-from-url';
import { createRouterCacheKey } from './create-router-cache-key';
import { fetchServerResponse } from './fetch-server-response';
import { dispatchAppRouterAction } from '../use-action-queue';
import { ACTION_SERVER_PATCH } from './router-reducer-types';
import { isNavigatingToNewRootLayout } from './is-navigating-to-new-root-layout';
import { DYNAMIC_STALETIME_MS } from './reducers/navigate-reducer';
import { convertServerPatchToFullTree } from '../segment-cache/navigation';
export var FreshnessPolicy = /*#__PURE__*/ function(FreshnessPolicy) {
    FreshnessPolicy[FreshnessPolicy["Default"] = 0] = "Default";
    FreshnessPolicy[FreshnessPolicy["Hydration"] = 1] = "Hydration";
    FreshnessPolicy[FreshnessPolicy["HistoryTraversal"] = 2] = "HistoryTraversal";
    FreshnessPolicy[FreshnessPolicy["RefreshAll"] = 3] = "RefreshAll";
    FreshnessPolicy[FreshnessPolicy["HMRRefresh"] = 4] = "HMRRefresh";
    return FreshnessPolicy;
}({});
const noop = ()=>{};
export function createInitialCacheNodeForHydration(navigatedAt, initialTree, seedData, seedHead) {
    // Create the initial cache node tree, using the data embedded into the
    // HTML document.
    const accumulation = {
        scrollableSegments: null,
        separateRefreshUrls: null
    };
    const task = createCacheNodeOnNavigation(navigatedAt, initialTree, undefined, 1, seedData, seedHead, null, null, false, null, null, false, accumulation);
    // NOTE: We intentionally don't check if any data needs to be fetched from the
    // server. We assume the initial hydration payload is sufficient to render
    // the page.
    //
    // The completeness of the initial data is an important property that we rely
    // on as a last-ditch mechanism for recovering the app; we must always be able
    // to reload a fresh HTML document to get to a consistent state.
    //
    // In the future, there may be cases where the server intentionally sends
    // partial data and expects the client to fill in the rest, in which case this
    // logic may change. (There already is a similar case where the server sends
    // _no_ hydration data in the HTML document at all, and the client fetches it
    // separately, but that's different because we still end up hydrating with a
    // complete tree.)
    return task.node;
}
// Creates a new Cache Node tree (i.e. copy-on-write) that represents the
// optimistic result of a navigation, using both the current Cache Node tree and
// data that was prefetched prior to navigation.
//
// At the moment we call this function, we haven't yet received the navigation
// response from the server. It could send back something completely different
// from the tree that was prefetched — due to rewrites, default routes, parallel
// routes, etc.
//
// But in most cases, it will return the same tree that we prefetched, just with
// the dynamic holes filled in. So we optimistically assume this will happen,
// and accept that the real result could be arbitrarily different.
//
// We'll reuse anything that was already in the previous tree, since that's what
// the server does.
//
// New segments (ones that don't appear in the old tree) are assigned an
// unresolved promise. The data for these promises will be fulfilled later, when
// the navigation response is received.
//
// The tree can be rendered immediately after it is created (that's why this is
// a synchronous function). Any new trees that do not have prefetch data will
// suspend during rendering, until the dynamic data streams in.
//
// Returns a Task object, which contains both the updated Cache Node and a path
// to the pending subtrees that need to be resolved by the navigation response.
//
// A return value of `null` means there were no changes, and the previous tree
// can be reused without initiating a server request.
export function startPPRNavigation(navigatedAt, oldUrl, oldCacheNode, oldRouterState, newRouterState, freshness, seedData, seedHead, prefetchData, prefetchHead, isPrefetchHeadPartial, isSamePageNavigation, accumulation) {
    const didFindRootLayout = false;
    const parentNeedsDynamicRequest = false;
    const parentRefreshUrl = null;
    return updateCacheNodeOnNavigation(navigatedAt, oldUrl, oldCacheNode !== null ? oldCacheNode : undefined, oldRouterState, newRouterState, freshness, didFindRootLayout, seedData, seedHead, prefetchData, prefetchHead, isPrefetchHeadPartial, isSamePageNavigation, null, null, parentNeedsDynamicRequest, parentRefreshUrl, accumulation);
}
function updateCacheNodeOnNavigation(navigatedAt, oldUrl, oldCacheNode, oldRouterState, newRouterState, freshness, didFindRootLayout, seedData, seedHead, prefetchData, prefetchHead, isPrefetchHeadPartial, isSamePageNavigation, parentSegmentPath, parentParallelRouteKey, parentNeedsDynamicRequest, parentRefreshUrl, accumulation) {
    // Check if this segment matches the one in the previous route.
    const oldSegment = oldRouterState[0];
    const newSegment = newRouterState[0];
    if (!matchSegment(newSegment, oldSegment)) {
        // This segment does not match the previous route. We're now entering the
        // new part of the target route. Switch to the "create" path.
        if (// Check if the route tree changed before we reached a layout. (The
        // highest-level layout in a route tree is referred to as the "root"
        // layout.) This could mean that we're navigating between two different
        // root layouts. When this happens, we perform a full-page (MPA-style)
        // navigation.
        //
        // However, the algorithm for deciding where to start rendering a route
        // (i.e. the one performed in order to reach this function) is stricter
        // than the one used to detect a change in the root layout. So just
        // because we're re-rendering a segment outside of the root layout does
        // not mean we should trigger a full-page navigation.
        //
        // Specifically, we handle dynamic parameters differently: two segments
        // are considered the same even if their parameter values are different.
        //
        // Refer to isNavigatingToNewRootLayout for details.
        //
        // Note that we only have to perform this extra traversal if we didn't
        // already discover a root layout in the part of the tree that is
        // unchanged. We also only need to compare the subtree that is not
        // shared. In the common case, this branch is skipped completely.
        !didFindRootLayout && isNavigatingToNewRootLayout(oldRouterState, newRouterState) || // The global Not Found route (app/global-not-found.tsx) is a special
        // case, because it acts like a root layout, but in the router tree, it
        // is rendered in the same position as app/layout.tsx.
        //
        // Any navigation to the global Not Found route should trigger a
        // full-page navigation.
        //
        // TODO: We should probably model this by changing the key of the root
        // segment when this happens. Then the root layout check would work
        // as expected, without a special case.
        newSegment === NOT_FOUND_SEGMENT_KEY) {
            return null;
        }
        if (parentSegmentPath === null || parentParallelRouteKey === null) {
            // The root should never mismatch. If it does, it suggests an internal
            // Next.js error, or a malformed server response. Trigger a full-
            // page navigation.
            return null;
        }
        return createCacheNodeOnNavigation(navigatedAt, newRouterState, oldCacheNode, freshness, seedData, seedHead, prefetchData, prefetchHead, isPrefetchHeadPartial, parentSegmentPath, parentParallelRouteKey, parentNeedsDynamicRequest, accumulation);
    }
    // TODO: The segment paths are tracked so that LayoutRouter knows which
    // segments to scroll to after a navigation. But we should just mark this
    // information on the CacheNode directly. It used to be necessary to do this
    // separately because CacheNodes were created lazily during render, not when
    // rather than when creating the route tree.
    const segmentPath = parentParallelRouteKey !== null && parentSegmentPath !== null ? parentSegmentPath.concat([
        parentParallelRouteKey,
        newSegment
    ]) : [];
    const newRouterStateChildren = newRouterState[1];
    const oldRouterStateChildren = oldRouterState[1];
    const seedDataChildren = seedData !== null ? seedData[1] : null;
    const prefetchDataChildren = prefetchData !== null ? prefetchData[1] : null;
    // We're currently traversing the part of the tree that was also part of
    // the previous route. If we discover a root layout, then we don't need to
    // trigger an MPA navigation.
    const isRootLayout = newRouterState[4] === true;
    const childDidFindRootLayout = didFindRootLayout || isRootLayout;
    const oldParallelRoutes = oldCacheNode !== undefined ? oldCacheNode.parallelRoutes : undefined;
    // Clone the current set of segment children, even if they aren't active in
    // the new tree.
    // TODO: We currently retain all the inactive segments indefinitely, until
    // there's an explicit refresh, or a parent layout is lazily refreshed. We
    // rely on this for popstate navigations, which update the Router State Tree
    // but do not eagerly perform a data fetch, because they expect the segment
    // data to already be in the Cache Node tree. For highly static sites that
    // are mostly read-only, this may happen only rarely, causing memory to
    // leak. We should figure out a better model for the lifetime of inactive
    // segments, so we can maintain instant back/forward navigations without
    // leaking memory indefinitely.
    let shouldDropSiblingCaches = false;
    let shouldRefreshDynamicData = false;
    switch(freshness){
        case 0:
        case 2:
        case 1:
            // We should never drop dynamic data in shared layouts, except during
            // a refresh.
            shouldDropSiblingCaches = false;
            shouldRefreshDynamicData = false;
            break;
        case 3:
        case 4:
            shouldDropSiblingCaches = true;
            shouldRefreshDynamicData = true;
            break;
        default:
            freshness;
            break;
    }
    const newParallelRoutes = new Map(shouldDropSiblingCaches ? undefined : oldParallelRoutes);
    // TODO: We're not consistent about how we do this check. Some places
    // check if the segment starts with PAGE_SEGMENT_KEY, but most seem to
    // check if there any any children, which is why I'm doing it here. We
    // should probably encode an empty children set as `null` though. Either
    // way, we should update all the checks to be consistent.
    const isLeafSegment = Object.keys(newRouterStateChildren).length === 0;
    // Get the data for this segment. Since it was part of the previous route,
    // usually we just clone the data from the old CacheNode. However, during a
    // refresh or a revalidation, there won't be any existing CacheNode. So we
    // may need to consult the prefetch cache, like we would for a new segment.
    let newCacheNode;
    let needsDynamicRequest;
    if (oldCacheNode !== undefined && !shouldRefreshDynamicData && // During a same-page navigation, we always refetch the page segments
    !(isLeafSegment && isSamePageNavigation)) {
        // Reuse the existing CacheNode
        const dropPrefetchRsc = false;
        newCacheNode = reuseDynamicCacheNode(dropPrefetchRsc, oldCacheNode, newParallelRoutes);
        needsDynamicRequest = false;
    } else if (seedData !== null && seedData[0] !== null) {
        // If this navigation was the result of an action, then check if the
        // server sent back data in the action response. We should favor using
        // that, rather than performing a separate request. This is both better
        // for performance and it's more likely to be consistent with any
        // writes that were just performed by the action, compared to a
        // separate request.
        const seedRsc = seedData[0];
        const seedLoading = seedData[2];
        const isSeedRscPartial = false;
        const isSeedHeadPartial = seedHead === null;
        newCacheNode = readCacheNodeFromSeedData(seedRsc, seedLoading, isSeedRscPartial, seedHead, isSeedHeadPartial, isLeafSegment, newParallelRoutes, navigatedAt);
        needsDynamicRequest = isLeafSegment && isSeedHeadPartial;
    } else if (prefetchData !== null) {
        // Consult the prefetch cache.
        const prefetchRsc = prefetchData[0];
        const prefetchLoading = prefetchData[2];
        const isPrefetchRSCPartial = prefetchData[3];
        newCacheNode = readCacheNodeFromSeedData(prefetchRsc, prefetchLoading, isPrefetchRSCPartial, prefetchHead, isPrefetchHeadPartial, isLeafSegment, newParallelRoutes, navigatedAt);
        needsDynamicRequest = isPrefetchRSCPartial || isLeafSegment && isPrefetchHeadPartial;
    } else {
        // Spawn a request to fetch new data from the server.
        newCacheNode = spawnNewCacheNode(newParallelRoutes, isLeafSegment, navigatedAt, freshness);
        needsDynamicRequest = true;
    }
    // During a refresh navigation, there's a special case that happens when
    // entering a "default" slot. The default slot may not be part of the
    // current route; it may have been reused from an older route. If so,
    // we need to fetch its data from the old route's URL rather than current
    // route's URL. Keep track of this as we traverse the tree.
    const href = newRouterState[2];
    const refreshUrl = typeof href === 'string' && newRouterState[3] === 'refresh' ? // refresh URL as we continue traversing the tree.
    href : parentRefreshUrl;
    // If this segment itself needs to fetch new data from the server, then by
    // definition it is being refreshed. Track its refresh URL so we know which
    // URL to request the data from.
    if (needsDynamicRequest && refreshUrl !== null) {
        accumulateRefreshUrl(accumulation, refreshUrl);
    }
    // As we diff the trees, we may sometimes modify (copy-on-write, not mutate)
    // the Route Tree that was returned by the server — for example, in the case
    // of default parallel routes, we preserve the currently active segment. To
    // avoid mutating the original tree, we clone the router state children along
    // the return path.
    let patchedRouterStateChildren = {};
    let taskChildren = null;
    // Most navigations require a request to fetch additional data from the
    // server, either because the data was not already prefetched, or because the
    // target route contains dynamic data that cannot be prefetched.
    //
    // However, if the target route is fully static, and it's already completely
    // loaded into the segment cache, then we can skip the server request.
    //
    // This starts off as `false`, and is set to `true` if any of the child
    // routes requires a dynamic request.
    let childNeedsDynamicRequest = false;
    // As we traverse the children, we'll construct a FlightRouterState that can
    // be sent to the server to request the dynamic data. If it turns out that
    // nothing in the subtree is dynamic (i.e. childNeedsDynamicRequest is false
    // at the end), then this will be discarded.
    // TODO: We can probably optimize the format of this data structure to only
    // include paths that are dynamic. Instead of reusing the
    // FlightRouterState type.
    let dynamicRequestTreeChildren = {};
    for(let parallelRouteKey in newRouterStateChildren){
        let newRouterStateChild = newRouterStateChildren[parallelRouteKey];
        const oldRouterStateChild = oldRouterStateChildren[parallelRouteKey];
        if (oldRouterStateChild === undefined) {
            // This should never happen, but if it does, it suggests a malformed
            // server response. Trigger a full-page navigation.
            return null;
        }
        const oldSegmentMapChild = oldParallelRoutes !== undefined ? oldParallelRoutes.get(parallelRouteKey) : undefined;
        let seedDataChild = seedDataChildren !== null ? seedDataChildren[parallelRouteKey] : null;
        let prefetchDataChild = prefetchDataChildren !== null ? prefetchDataChildren[parallelRouteKey] : null;
        let newSegmentChild = newRouterStateChild[0];
        let seedHeadChild = seedHead;
        let prefetchHeadChild = prefetchHead;
        let isPrefetchHeadPartialChild = isPrefetchHeadPartial;
        if (// Skip this branch during a history traversal. We restore the tree that
        // was stashed in the history entry as-is.
        freshness !== 2 && newSegmentChild === DEFAULT_SEGMENT_KEY) {
            // This is a "default" segment. These are never sent by the server during
            // a soft navigation; instead, the client reuses whatever segment was
            // already active in that slot on the previous route.
            newRouterStateChild = reuseActiveSegmentInDefaultSlot(oldUrl, oldRouterStateChild);
            newSegmentChild = newRouterStateChild[0];
            // Since we're switching to a different route tree, these are no
            // longer valid, because they correspond to the outer tree.
            seedDataChild = null;
            seedHeadChild = null;
            prefetchDataChild = null;
            prefetchHeadChild = null;
            isPrefetchHeadPartialChild = false;
        }
        const newSegmentKeyChild = createRouterCacheKey(newSegmentChild);
        const oldCacheNodeChild = oldSegmentMapChild !== undefined ? oldSegmentMapChild.get(newSegmentKeyChild) : undefined;
        const taskChild = updateCacheNodeOnNavigation(navigatedAt, oldUrl, oldCacheNodeChild, oldRouterStateChild, newRouterStateChild, freshness, childDidFindRootLayout, seedDataChild ?? null, seedHeadChild, prefetchDataChild ?? null, prefetchHeadChild, isPrefetchHeadPartialChild, isSamePageNavigation, segmentPath, parallelRouteKey, parentNeedsDynamicRequest || needsDynamicRequest, refreshUrl, accumulation);
        if (taskChild === null) {
            // One of the child tasks discovered a change to the root layout.
            // Immediately unwind from this recursive traversal. This will trigger a
            // full-page navigation.
            return null;
        }
        // Recursively propagate up the child tasks.
        if (taskChildren === null) {
            taskChildren = new Map();
        }
        taskChildren.set(parallelRouteKey, taskChild);
        const newCacheNodeChild = taskChild.node;
        if (newCacheNodeChild !== null) {
            const newSegmentMapChild = new Map(shouldDropSiblingCaches ? undefined : oldSegmentMapChild);
            newSegmentMapChild.set(newSegmentKeyChild, newCacheNodeChild);
            newParallelRoutes.set(parallelRouteKey, newSegmentMapChild);
        }
        // The child tree's route state may be different from the prefetched
        // route sent by the server. We need to clone it as we traverse back up
        // the tree.
        const taskChildRoute = taskChild.route;
        patchedRouterStateChildren[parallelRouteKey] = taskChildRoute;
        const dynamicRequestTreeChild = taskChild.dynamicRequestTree;
        if (dynamicRequestTreeChild !== null) {
            // Something in the child tree is dynamic.
            childNeedsDynamicRequest = true;
            dynamicRequestTreeChildren[parallelRouteKey] = dynamicRequestTreeChild;
        } else {
            dynamicRequestTreeChildren[parallelRouteKey] = taskChildRoute;
        }
    }
    return {
        status: needsDynamicRequest ? 0 : 1,
        route: patchRouterStateWithNewChildren(newRouterState, patchedRouterStateChildren),
        node: newCacheNode,
        dynamicRequestTree: createDynamicRequestTree(newRouterState, dynamicRequestTreeChildren, needsDynamicRequest, childNeedsDynamicRequest, parentNeedsDynamicRequest),
        refreshUrl,
        children: taskChildren
    };
}
function createCacheNodeOnNavigation(navigatedAt, newRouterState, oldCacheNode, freshness, seedData, seedHead, prefetchData, prefetchHead, isPrefetchHeadPartial, parentSegmentPath, parentParallelRouteKey, parentNeedsDynamicRequest, accumulation) {
    // Same traversal as updateCacheNodeNavigation, but simpler. We switch to this
    // path once we reach the part of the tree that was not in the previous route.
    // We don't need to diff against the old tree, we just need to create a new
    // one. We also don't need to worry about any refresh-related logic.
    //
    // For the most part, this is a subset of updateCacheNodeOnNavigation, so any
    // change that happens in this function likely needs to be applied to that
    // one, too. However there are some places where the behavior intentionally
    // diverges, which is why we keep them separate.
    const newSegment = newRouterState[0];
    const segmentPath = parentParallelRouteKey !== null && parentSegmentPath !== null ? parentSegmentPath.concat([
        parentParallelRouteKey,
        newSegment
    ]) : [];
    const newRouterStateChildren = newRouterState[1];
    const prefetchDataChildren = prefetchData !== null ? prefetchData[1] : null;
    const seedDataChildren = seedData !== null ? seedData[1] : null;
    const oldParallelRoutes = oldCacheNode !== undefined ? oldCacheNode.parallelRoutes : undefined;
    let shouldDropSiblingCaches = false;
    let shouldRefreshDynamicData = false;
    let dropPrefetchRsc = false;
    switch(freshness){
        case 0:
            // We should never drop dynamic data in sibling caches except during
            // a refresh.
            shouldDropSiblingCaches = false;
            // Only reuse the dynamic data if experimental.staleTimes.dynamic config
            // is set, and the data is not stale. (This is not a recommended API with
            // Cache Components, but it's supported for backwards compatibility. Use
            // cacheLife instead.)
            //
            // DYNAMIC_STALETIME_MS defaults to 0, but it can be increased.
            shouldRefreshDynamicData = oldCacheNode === undefined || navigatedAt - oldCacheNode.navigatedAt >= DYNAMIC_STALETIME_MS;
            dropPrefetchRsc = false;
            break;
        case 1:
            // During hydration, we assume the data sent by the server is both
            // consistent and complete.
            shouldRefreshDynamicData = false;
            shouldDropSiblingCaches = false;
            dropPrefetchRsc = false;
            break;
        case 2:
            // During back/forward navigations, we reuse the dynamic data regardless
            // of how stale it may be.
            shouldRefreshDynamicData = false;
            shouldRefreshDynamicData = false;
            // Only show prefetched data if the dynamic data is still pending. This
            // avoids a flash back to the prefetch state in a case where it's highly
            // likely to have already streamed in.
            //
            // Tehnically, what we're actually checking is whether the dynamic network
            // response was received. But since it's a streaming response, this does
            // not mean that all the dynamic data has fully streamed in. It just means
            // that _some_ of the dynamic data was received. But as a heuristic, we
            // assume that the rest dynamic data will stream in quickly, so it's still
            // better to skip the prefetch state.
            if (oldCacheNode !== undefined) {
                const oldRsc = oldCacheNode.rsc;
                const oldRscDidResolve = !isDeferredRsc(oldRsc) || oldRsc.status !== 'pending';
                dropPrefetchRsc = oldRscDidResolve;
            } else {
                dropPrefetchRsc = false;
            }
            break;
        case 3:
        case 4:
            // Drop all dynamic data.
            shouldRefreshDynamicData = true;
            shouldDropSiblingCaches = true;
            dropPrefetchRsc = false;
            break;
        default:
            freshness;
            break;
    }
    const newParallelRoutes = new Map(shouldDropSiblingCaches ? undefined : oldParallelRoutes);
    const isLeafSegment = Object.keys(newRouterStateChildren).length === 0;
    if (isLeafSegment) {
        // The segment path of every leaf segment (i.e. page) is collected into
        // a result array. This is used by the LayoutRouter to scroll to ensure that
        // new pages are visible after a navigation.
        //
        // This only happens for new pages, not for refreshed pages.
        //
        // TODO: We should use a string to represent the segment path instead of
        // an array. We already use a string representation for the path when
        // accessing the Segment Cache, so we can use the same one.
        if (accumulation.scrollableSegments === null) {
            accumulation.scrollableSegments = [];
        }
        accumulation.scrollableSegments.push(segmentPath);
    }
    let newCacheNode;
    let needsDynamicRequest;
    if (!shouldRefreshDynamicData && oldCacheNode !== undefined) {
        // Reuse the existing CacheNode
        newCacheNode = reuseDynamicCacheNode(dropPrefetchRsc, oldCacheNode, newParallelRoutes);
        needsDynamicRequest = false;
    } else if (seedData !== null && seedData[0] !== null) {
        // If this navigation was the result of an action, then check if the
        // server sent back data in the action response. We should favor using
        // that, rather than performing a separate request. This is both better
        // for performance and it's more likely to be consistent with any
        // writes that were just performed by the action, compared to a
        // separate request.
        const seedRsc = seedData[0];
        const seedLoading = seedData[2];
        const isSeedRscPartial = false;
        const isSeedHeadPartial = seedHead === null && freshness !== 1;
        newCacheNode = readCacheNodeFromSeedData(seedRsc, seedLoading, isSeedRscPartial, seedHead, isSeedHeadPartial, isLeafSegment, newParallelRoutes, navigatedAt);
        needsDynamicRequest = isLeafSegment && isSeedHeadPartial;
    } else if (freshness === 1 && isLeafSegment && seedHead !== null) {
        // This is another weird case related to "not found" pages and hydration.
        // There will be a head sent by the server, but no page seed data.
        // TODO: We really should get rid of all these "not found" specific quirks
        // and make sure the tree is always consistent.
        const seedRsc = null;
        const seedLoading = null;
        const isSeedRscPartial = false;
        const isSeedHeadPartial = false;
        newCacheNode = readCacheNodeFromSeedData(seedRsc, seedLoading, isSeedRscPartial, seedHead, isSeedHeadPartial, isLeafSegment, newParallelRoutes, navigatedAt);
        needsDynamicRequest = false;
    } else if (freshness !== 1 && prefetchData !== null) {
        // Consult the prefetch cache.
        const prefetchRsc = prefetchData[0];
        const prefetchLoading = prefetchData[2];
        const isPrefetchRSCPartial = prefetchData[3];
        newCacheNode = readCacheNodeFromSeedData(prefetchRsc, prefetchLoading, isPrefetchRSCPartial, prefetchHead, isPrefetchHeadPartial, isLeafSegment, newParallelRoutes, navigatedAt);
        needsDynamicRequest = isPrefetchRSCPartial || isLeafSegment && isPrefetchHeadPartial;
    } else {
        // Spawn a request to fetch new data from the server.
        newCacheNode = spawnNewCacheNode(newParallelRoutes, isLeafSegment, navigatedAt, freshness);
        needsDynamicRequest = true;
    }
    let patchedRouterStateChildren = {};
    let taskChildren = null;
    let childNeedsDynamicRequest = false;
    let dynamicRequestTreeChildren = {};
    for(let parallelRouteKey in newRouterStateChildren){
        const newRouterStateChild = newRouterStateChildren[parallelRouteKey];
        const oldSegmentMapChild = oldParallelRoutes !== undefined ? oldParallelRoutes.get(parallelRouteKey) : undefined;
        const seedDataChild = seedDataChildren !== null ? seedDataChildren[parallelRouteKey] : null;
        const prefetchDataChild = prefetchDataChildren !== null ? prefetchDataChildren[parallelRouteKey] : null;
        const newSegmentChild = newRouterStateChild[0];
        const newSegmentKeyChild = createRouterCacheKey(newSegmentChild);
        const oldCacheNodeChild = oldSegmentMapChild !== undefined ? oldSegmentMapChild.get(newSegmentKeyChild) : undefined;
        const taskChild = createCacheNodeOnNavigation(navigatedAt, newRouterStateChild, oldCacheNodeChild, freshness, seedDataChild ?? null, seedHead, prefetchDataChild ?? null, prefetchHead, isPrefetchHeadPartial, segmentPath, parallelRouteKey, parentNeedsDynamicRequest || needsDynamicRequest, accumulation);
        if (taskChildren === null) {
            taskChildren = new Map();
        }
        taskChildren.set(parallelRouteKey, taskChild);
        const newCacheNodeChild = taskChild.node;
        if (newCacheNodeChild !== null) {
            const newSegmentMapChild = new Map(shouldDropSiblingCaches ? undefined : oldSegmentMapChild);
            newSegmentMapChild.set(newSegmentKeyChild, newCacheNodeChild);
            newParallelRoutes.set(parallelRouteKey, newSegmentMapChild);
        }
        const taskChildRoute = taskChild.route;
        patchedRouterStateChildren[parallelRouteKey] = taskChildRoute;
        const dynamicRequestTreeChild = taskChild.dynamicRequestTree;
        if (dynamicRequestTreeChild !== null) {
            childNeedsDynamicRequest = true;
            dynamicRequestTreeChildren[parallelRouteKey] = dynamicRequestTreeChild;
        } else {
            dynamicRequestTreeChildren[parallelRouteKey] = taskChildRoute;
        }
    }
    return {
        status: needsDynamicRequest ? 0 : 1,
        route: patchRouterStateWithNewChildren(newRouterState, patchedRouterStateChildren),
        node: newCacheNode,
        dynamicRequestTree: createDynamicRequestTree(newRouterState, dynamicRequestTreeChildren, needsDynamicRequest, childNeedsDynamicRequest, parentNeedsDynamicRequest),
        // This route is not part of the current tree, so there's no reason to
        // track the refresh URL.
        refreshUrl: null,
        children: taskChildren
    };
}
function patchRouterStateWithNewChildren(baseRouterState, newChildren) {
    const clone = [
        baseRouterState[0],
        newChildren
    ];
    // Based on equivalent logic in apply-router-state-patch-to-tree, but should
    // confirm whether we need to copy all of these fields. Not sure the server
    // ever sends, e.g. the refetch marker.
    if (2 in baseRouterState) {
        clone[2] = baseRouterState[2];
    }
    if (3 in baseRouterState) {
        clone[3] = baseRouterState[3];
    }
    if (4 in baseRouterState) {
        clone[4] = baseRouterState[4];
    }
    return clone;
}
function createDynamicRequestTree(newRouterState, dynamicRequestTreeChildren, needsDynamicRequest, childNeedsDynamicRequest, parentNeedsDynamicRequest) {
    // Create a FlightRouterState that instructs the server how to render the
    // requested segment.
    //
    // Or, if neither this segment nor any of the children require a new data,
    // then we return `null` to skip the request.
    let dynamicRequestTree = null;
    if (needsDynamicRequest) {
        dynamicRequestTree = patchRouterStateWithNewChildren(newRouterState, dynamicRequestTreeChildren);
        // The "refetch" marker is set on the top-most segment that requires new
        // data. We can omit it if a parent was already marked.
        if (!parentNeedsDynamicRequest) {
            dynamicRequestTree[3] = 'refetch';
        }
    } else if (childNeedsDynamicRequest) {
        // This segment does not request new data, but at least one of its
        // children does.
        dynamicRequestTree = patchRouterStateWithNewChildren(newRouterState, dynamicRequestTreeChildren);
    } else {
        dynamicRequestTree = null;
    }
    return dynamicRequestTree;
}
function accumulateRefreshUrl(accumulation, refreshUrl) {
    // This is a refresh navigation, and we're inside a "default" slot that's
    // not part of the current route; it was reused from an older route. In
    // order to get fresh data for this reused route, we need to issue a
    // separate request using the old route's URL.
    //
    // Track these extra URLs in the accumulated result. Later, we'll construct
    // an appropriate request for each unique URL in the final set. The reason
    // we don't do it immediately here is so we can deduplicate multiple
    // instances of the same URL into a single request. See
    // listenForDynamicRequest for more details.
    const separateRefreshUrls = accumulation.separateRefreshUrls;
    if (separateRefreshUrls === null) {
        accumulation.separateRefreshUrls = new Set([
            refreshUrl
        ]);
    } else {
        separateRefreshUrls.add(refreshUrl);
    }
}
function reuseActiveSegmentInDefaultSlot(oldUrl, oldRouterState) {
    // This is a "default" segment. These are never sent by the server during a
    // soft navigation; instead, the client reuses whatever segment was already
    // active in that slot on the previous route. This means if we later need to
    // refresh the segment, it will have to be refetched from the previous route's
    // URL. We store it in the Flight Router State.
    //
    // TODO: We also mark the segment with a "refresh" marker but I think we can
    // get rid of that eventually by making sure we only add URLs to page segments
    // that are reused. Then the presence of the URL alone is enough.
    let reusedRouterState;
    const oldRefreshMarker = oldRouterState[3];
    if (oldRefreshMarker === 'refresh') {
        // This segment was already reused from an even older route. Keep its
        // existing URL and refresh marker.
        reusedRouterState = oldRouterState;
    } else {
        // This segment was not previously reused, and it's not on the new route.
        // So it must have been delivered in the old route.
        reusedRouterState = patchRouterStateWithNewChildren(oldRouterState, oldRouterState[1]);
        reusedRouterState[2] = createHrefFromUrl(oldUrl);
        reusedRouterState[3] = 'refresh';
    }
    return reusedRouterState;
}
function reuseDynamicCacheNode(dropPrefetchRsc, existingCacheNode, parallelRoutes) {
    // Clone an existing CacheNode's data, with (possibly) new children.
    const cacheNode = {
        rsc: existingCacheNode.rsc,
        prefetchRsc: dropPrefetchRsc ? null : existingCacheNode.prefetchRsc,
        head: existingCacheNode.head,
        prefetchHead: dropPrefetchRsc ? null : existingCacheNode.prefetchHead,
        loading: existingCacheNode.loading,
        parallelRoutes,
        // Don't update the navigatedAt timestamp, since we're reusing
        // existing data.
        navigatedAt: existingCacheNode.navigatedAt
    };
    return cacheNode;
}
function readCacheNodeFromSeedData(seedRsc, seedLoading, isSeedRscPartial, seedHead, isSeedHeadPartial, isPageSegment, parallelRoutes, navigatedAt) {
    // TODO: Currently this is threaded through the navigation logic using the
    // CacheNodeSeedData type, but in the future this will read directly from
    // the Segment Cache. See readRenderSnapshotFromCache.
    let rsc;
    let prefetchRsc;
    if (isSeedRscPartial) {
        // The prefetched data contains dynamic holes. Create a pending promise that
        // will be fulfilled when the dynamic data is received from the server.
        prefetchRsc = seedRsc;
        rsc = createDeferredRsc();
    } else {
        // The prefetched data is complete. Use it directly.
        prefetchRsc = null;
        rsc = seedRsc;
    }
    // If this is a page segment, also read the head.
    let prefetchHead;
    let head;
    if (isPageSegment) {
        if (isSeedHeadPartial) {
            prefetchHead = seedHead;
            head = createDeferredRsc();
        } else {
            prefetchHead = null;
            head = seedHead;
        }
    } else {
        prefetchHead = null;
        head = null;
    }
    const cacheNode = {
        rsc,
        prefetchRsc,
        head,
        prefetchHead,
        // TODO: Technically, a loading boundary could contain dynamic data. We
        // should have separate `loading` and `prefetchLoading` fields to handle
        // this, like we do for the segment data and head.
        loading: seedLoading,
        parallelRoutes,
        navigatedAt
    };
    return cacheNode;
}
function spawnNewCacheNode(parallelRoutes, isLeafSegment, navigatedAt, freshness) {
    // We should never spawn network requests during hydration. We must treat the
    // initial payload as authoritative, because the initial page load is used
    // as a last-ditch mechanism for recovering the app.
    //
    // This is also an important safety check because if this leaks into the
    // server rendering path (which theoretically it never should because
    // the server payload should be consistent), the server would hang because
    // these promises would never resolve.
    //
    // TODO: There is an existing case where the global "not found" boundary
    // triggers this path. But it does render correctly despite that. That's an
    // unusual render path so it's not surprising, but we should look into
    // modeling it in a more consistent way. See also the /_notFound special
    // case in updateCacheNodeOnNavigation.
    const isHydration = freshness === 1;
    const cacheNode = {
        rsc: !isHydration ? createDeferredRsc() : null,
        prefetchRsc: null,
        head: !isHydration && isLeafSegment ? createDeferredRsc() : null,
        prefetchHead: null,
        loading: !isHydration ? createDeferredRsc() : null,
        parallelRoutes,
        navigatedAt
    };
    return cacheNode;
}
// Represents whether the previuos navigation resulted in a route tree mismatch.
// A mismatch results in a refresh of the page. If there are two successive
// mismatches, we will fall back to an MPA navigation, to prevent a retry loop.
let previousNavigationDidMismatch = false;
// Writes a dynamic server response into the tree created by
// updateCacheNodeOnNavigation. All pending promises that were spawned by the
// navigation will be resolved, either with dynamic data from the server, or
// `null` to indicate that the data is missing.
//
// A `null` value will trigger a lazy fetch during render, which will then patch
// up the tree using the same mechanism as the non-PPR implementation
// (serverPatchReducer).
//
// Usually, the server will respond with exactly the subset of data that we're
// waiting for — everything below the nearest shared layout. But technically,
// the server can return anything it wants.
//
// This does _not_ create a new tree; it modifies the existing one in place.
// Which means it must follow the Suspense rules of cache safety.
export function spawnDynamicRequests(task, primaryUrl, nextUrl, freshnessPolicy, accumulation) {
    const dynamicRequestTree = task.dynamicRequestTree;
    if (dynamicRequestTree === null) {
        // This navigation was fully cached. There are no dynamic requests to spawn.
        previousNavigationDidMismatch = false;
        return;
    }
    // This is intentionally not an async function to discourage the caller from
    // awaiting the result. Any subsequent async operations spawned by this
    // function should result in a separate navigation task, rather than
    // block the original one.
    //
    // In this function we spawn (but do not await) all the network requests that
    // block the navigation, and collect the promises. The next function,
    // `finishNavigationTask`, can await the promises in any order without
    // accidentally introducing a network waterfall.
    const primaryRequestPromise = fetchMissingDynamicData(task, dynamicRequestTree, primaryUrl, nextUrl, freshnessPolicy);
    const separateRefreshUrls = accumulation.separateRefreshUrls;
    let refreshRequestPromises = null;
    if (separateRefreshUrls !== null) {
        // There are multiple URLs that we need to request the data from. This
        // happens when a "default" parallel route slot is present in the tree, and
        // its data cannot be fetched from the current route. We need to split the
        // combined dynamic request tree into separate requests per URL.
        // TODO: Create a scoped dynamic request tree that omits anything that
        // is not relevant to the given URL. Without doing this, the server may
        // sometimes render more data than necessary; this is not a regression
        // compared to the pre-Segment Cache implementation, though, just an
        // optimization we can make in the future.
        // Construct a request tree for each additional refresh URL. This will
        // prune away everything except the parts of the tree that match the
        // given refresh URL.
        refreshRequestPromises = [];
        const canonicalUrl = createHrefFromUrl(primaryUrl);
        for (const refreshUrl of separateRefreshUrls){
            if (refreshUrl === canonicalUrl) {
                continue;
            }
            // TODO: Create a scoped dynamic request tree that omits anything that
            // is not relevant to the given URL. Without doing this, the server may
            // sometimes render more data than necessary; this is not a regression
            // compared to the pre-Segment Cache implementation, though, just an
            // optimization we can make in the future.
            // const scopedDynamicRequestTree = splitTaskByURL(task, refreshUrl)
            const scopedDynamicRequestTree = dynamicRequestTree;
            if (scopedDynamicRequestTree !== null) {
                refreshRequestPromises.push(fetchMissingDynamicData(task, scopedDynamicRequestTree, new URL(refreshUrl, location.origin), // TODO: Just noticed that this should actually the Next-Url at the
                // time the refresh URL was set, not the current Next-Url. Need to
                // start tracking this alongside the refresh URL. In the meantime,
                // if a refresh fails due to a mismatch, it will trigger a
                // hard refresh.
                nextUrl, freshnessPolicy));
            }
        }
    }
    // Further async operations are moved into this separate function to
    // discourage sequential network requests.
    const voidPromise = finishNavigationTask(task, nextUrl, primaryRequestPromise, refreshRequestPromises);
    // `finishNavigationTask` is responsible for error handling, so we can attach
    // noop callbacks to this promise.
    voidPromise.then(noop, noop);
}
async function finishNavigationTask(task, nextUrl, primaryRequestPromise, refreshRequestPromises) {
    // Wait for all the requests to finish, or for the first one to fail.
    let exitStatus = await waitForRequestsToFinish(primaryRequestPromise, refreshRequestPromises);
    // Once the all the requests have finished, check the tree for any remaining
    // pending tasks. If anything is still pending, it means the server response
    // does not match the client, and we must refresh to get back to a consistent
    // state. We can skip this step if we already detected a mismatch during the
    // first phase; it doesn't matter in that case because we're going to refresh
    // the whole tree regardless.
    if (exitStatus === 0) {
        exitStatus = abortRemainingPendingTasks(task, null, null);
    }
    switch(exitStatus){
        case 0:
            {
                // The task has completely finished. There's no missing data. Exit.
                previousNavigationDidMismatch = false;
                return;
            }
        case 1:
            {
                // Some data failed to finish loading. Trigger a soft retry.
                // TODO: As an extra precaution against soft retry loops, consider
                // tracking whether a navigation was itself triggered by a retry. If two
                // happen in a row, fall back to a hard retry.
                const isHardRetry = false;
                const primaryRequestResult = await primaryRequestPromise;
                dispatchRetryDueToTreeMismatch(isHardRetry, primaryRequestResult.url, nextUrl, primaryRequestResult.seed, task.route);
                return;
            }
        case 2:
            {
                // Some data failed to finish loading in a non-recoverable way, such as a
                // network error. Trigger an MPA navigation.
                //
                // Hard navigating/refreshing is how we prevent an infinite retry loop
                // caused by a network error — when the network fails, we fall back to the
                // browser behavior for offline navigations. In the future, Next.js may
                // introduce its own custom handling of offline navigations, but that
                // doesn't exist yet.
                const isHardRetry = true;
                const primaryRequestResult = await primaryRequestPromise;
                dispatchRetryDueToTreeMismatch(isHardRetry, primaryRequestResult.url, nextUrl, primaryRequestResult.seed, task.route);
                return;
            }
        default:
            {
                return exitStatus;
            }
    }
}
function waitForRequestsToFinish(primaryRequestPromise, refreshRequestPromises) {
    // Custom async combinator logic. This could be replaced by Promise.any but
    // we don't assume that's available.
    //
    // Each promise resolves once the server responsds and the data is written
    // into the CacheNode tree. Resolve the combined promise once all the
    // requests finish.
    //
    // Or, resolve as soon as one of the requests fails, without waiting for the
    // others to finish.
    return new Promise((resolve)=>{
        const onFulfill = (result)=>{
            if (result.exitStatus === 0) {
                remainingCount--;
                if (remainingCount === 0) {
                    // All the requests finished successfully.
                    resolve(0);
                }
            } else {
                // One of the requests failed. Exit with a failing status.
                // NOTE: It's possible for one of the requests to fail with SoftRetry
                // and a later one to fail with HardRetry. In this case, we choose to
                // retry immediately, rather than delay the retry until all the requests
                // finish. If it fails again, we will hard retry on the next
                // attempt, anyway.
                resolve(result.exitStatus);
            }
        };
        // onReject shouldn't ever be called because fetchMissingDynamicData's
        // entire body is wrapped in a try/catch. This is just defensive.
        const onReject = ()=>resolve(2);
        // Attach the listeners to the promises.
        let remainingCount = 1;
        primaryRequestPromise.then(onFulfill, onReject);
        if (refreshRequestPromises !== null) {
            remainingCount += refreshRequestPromises.length;
            refreshRequestPromises.forEach((refreshRequestPromise)=>refreshRequestPromise.then(onFulfill, onReject));
        }
    });
}
function dispatchRetryDueToTreeMismatch(isHardRetry, retryUrl, retryNextUrl, seed, baseTree) {
    // If this is the second time in a row that a navigation resulted in a
    // mismatch, fall back to a hard (MPA) refresh.
    isHardRetry = isHardRetry || previousNavigationDidMismatch;
    previousNavigationDidMismatch = true;
    const retryAction = {
        type: ACTION_SERVER_PATCH,
        previousTree: baseTree,
        url: retryUrl,
        nextUrl: retryNextUrl,
        seed,
        mpa: isHardRetry
    };
    dispatchAppRouterAction(retryAction);
}
async function fetchMissingDynamicData(task, dynamicRequestTree, url, nextUrl, freshnessPolicy) {
    try {
        const result = await fetchServerResponse(url, {
            flightRouterState: dynamicRequestTree,
            nextUrl,
            isHmrRefresh: freshnessPolicy === 4
        });
        if (typeof result === 'string') {
            // fetchServerResponse will return an href to indicate that the SPA
            // navigation failed. For example, if the server triggered a hard
            // redirect, or the fetch request errored. Initiate an MPA navigation
            // to the given href.
            return {
                exitStatus: 2,
                url: new URL(result, location.origin),
                seed: null
            };
        }
        const seed = convertServerPatchToFullTree(task.route, result.flightData, result.renderedSearch);
        const didReceiveUnknownParallelRoute = writeDynamicDataIntoNavigationTask(task, seed.tree, seed.data, seed.head, result.debugInfo);
        return {
            exitStatus: didReceiveUnknownParallelRoute ? 1 : 0,
            url: new URL(result.canonicalUrl, location.origin),
            seed
        };
    } catch  {
        // This shouldn't happen because fetchServerResponse's entire body is
        // wrapped in a try/catch. If it does, though, it implies the server failed
        // to respond with any tree at all. So we must fall back to a hard retry.
        return {
            exitStatus: 2,
            url: url,
            seed: null
        };
    }
}
function writeDynamicDataIntoNavigationTask(task, serverRouterState, dynamicData, dynamicHead, debugInfo) {
    if (task.status === 0 && dynamicData !== null) {
        task.status = 1;
        finishPendingCacheNode(task.node, dynamicData, dynamicHead, debugInfo);
    }
    const taskChildren = task.children;
    const serverChildren = serverRouterState[1];
    const dynamicDataChildren = dynamicData !== null ? dynamicData[1] : null;
    // Detect whether the server sends a parallel route slot that the client
    // doesn't know about.
    let didReceiveUnknownParallelRoute = false;
    if (taskChildren !== null) {
        for(const parallelRouteKey in serverChildren){
            const serverRouterStateChild = serverChildren[parallelRouteKey];
            const dynamicDataChild = dynamicDataChildren !== null ? dynamicDataChildren[parallelRouteKey] : null;
            const taskChild = taskChildren.get(parallelRouteKey);
            if (taskChild === undefined) {
                // The server sent a child segment that the client doesn't know about.
                //
                // When we receive an unknown parallel route, we must consider it a
                // mismatch. This is unlike the case where the segment itself
                // mismatches, because multiple routes can be active simultaneously.
                // But a given layout should never have a mismatching set of
                // child slots.
                //
                // Theoretically, this should only happen in development during an HMR
                // refresh, because the set of parallel routes for a layout does not
                // change over the lifetime of a build/deployment. In production, we
                // should have already mismatched on either the build id or the segment
                // path. But as an extra precaution, we validate in prod, too.
                didReceiveUnknownParallelRoute = true;
            } else {
                const taskSegment = taskChild.route[0];
                if (matchSegment(serverRouterStateChild[0], taskSegment) && dynamicDataChild !== null && dynamicDataChild !== undefined) {
                    // Found a match for this task. Keep traversing down the task tree.
                    const childDidReceiveUnknownParallelRoute = writeDynamicDataIntoNavigationTask(taskChild, serverRouterStateChild, dynamicDataChild, dynamicHead, debugInfo);
                    if (childDidReceiveUnknownParallelRoute) {
                        didReceiveUnknownParallelRoute = true;
                    }
                }
            }
        }
    }
    return didReceiveUnknownParallelRoute;
}
function finishPendingCacheNode(cacheNode, dynamicData, dynamicHead, debugInfo) {
    // Writes a dynamic response into an existing Cache Node tree. This does _not_
    // create a new tree, it updates the existing tree in-place. So it must follow
    // the Suspense rules of cache safety — it can resolve pending promises, but
    // it cannot overwrite existing data. It can add segments to the tree (because
    // a missing segment will cause the layout router to suspend).
    // but it cannot delete them.
    //
    // We must resolve every promise in the tree, or else it will suspend
    // indefinitely. If we did not receive data for a segment, we will resolve its
    // data promise to `null` to trigger a lazy fetch during render.
    // Use the dynamic data from the server to fulfill the deferred RSC promise
    // on the Cache Node.
    const rsc = cacheNode.rsc;
    const dynamicSegmentData = dynamicData[0];
    if (dynamicSegmentData === null) {
        // This is an empty CacheNode; this particular server request did not
        // render this segment. There may be a separate pending request that will,
        // though, so we won't abort the task until all pending requests finish.
        return;
    }
    if (rsc === null) {
        // This is a lazy cache node. We can overwrite it. This is only safe
        // because we know that the LayoutRouter suspends if `rsc` is `null`.
        cacheNode.rsc = dynamicSegmentData;
    } else if (isDeferredRsc(rsc)) {
        // This is a deferred RSC promise. We can fulfill it with the data we just
        // received from the server. If it was already resolved by a different
        // navigation, then this does nothing because we can't overwrite data.
        rsc.resolve(dynamicSegmentData, debugInfo);
    } else {
    // This is not a deferred RSC promise, nor is it empty, so it must have
    // been populated by a different navigation. We must not overwrite it.
    }
    // If we navigated without a prefetch, then `loading` will be a deferred promise too.
    // Fulfill it using the dynamic response so that we can display the loading boundary.
    const loading = cacheNode.loading;
    if (isDeferredRsc(loading)) {
        const dynamicLoading = dynamicData[2];
        loading.resolve(dynamicLoading, debugInfo);
    }
    // Check if this is a leaf segment. If so, it will have a `head` property with
    // a pending promise that needs to be resolved with the dynamic head from
    // the server.
    const head = cacheNode.head;
    if (isDeferredRsc(head)) {
        head.resolve(dynamicHead, debugInfo);
    }
}
function abortRemainingPendingTasks(task, error, debugInfo) {
    let exitStatus;
    if (task.status === 0) {
        // The data for this segment is still missing.
        task.status = 2;
        abortPendingCacheNode(task.node, error, debugInfo);
        // If the server failed to fulfill the data for this segment, it implies
        // that the route tree received from the server mismatched the tree that
        // was previously prefetched.
        //
        // In an app with fully static routes and no proxy-driven redirects or
        // rewrites, this should never happen, because the route for a URL would
        // always be the same across multiple requests. So, this implies that some
        // runtime routing condition changed, likely in a proxy, without being
        // pushed to the client.
        //
        // When this happens, we treat this the same as a refresh(). The entire
        // tree will be re-rendered from the root.
        if (task.refreshUrl === null) {
            // Trigger a "soft" refresh. Essentially the same as calling `refresh()`
            // in a Server Action.
            exitStatus = 1;
        } else {
            // The mismatch was discovered inside an inactive parallel route. This
            // implies the inactive parallel route is no longer reachable at the URL
            // that originally rendered it. Fall back to an MPA refresh.
            // TODO: An alternative could be to trigger a soft refresh but to _not_
            // re-use the inactive parallel routes this time. Similar to what would
            // happen if were to do a hard refrehs, but without the HTML page.
            exitStatus = 2;
        }
    } else {
        // This segment finished. (An error here is treated as Done because they are
        // surfaced to the application during render.)
        exitStatus = 0;
    }
    const taskChildren = task.children;
    if (taskChildren !== null) {
        for (const [, taskChild] of taskChildren){
            const childExitStatus = abortRemainingPendingTasks(taskChild, error, debugInfo);
            // Propagate the exit status up the tree. The statuses are ordered by
            // their precedence.
            if (childExitStatus > exitStatus) {
                exitStatus = childExitStatus;
            }
        }
    }
    return exitStatus;
}
function abortPendingCacheNode(cacheNode, error, debugInfo) {
    const rsc = cacheNode.rsc;
    if (isDeferredRsc(rsc)) {
        if (error === null) {
            // This will trigger a lazy fetch during render.
            rsc.resolve(null, debugInfo);
        } else {
            // This will trigger an error during rendering.
            rsc.reject(error, debugInfo);
        }
    }
    const loading = cacheNode.loading;
    if (isDeferredRsc(loading)) {
        loading.resolve(null, debugInfo);
    }
    // Check if this is a leaf segment. If so, it will have a `head` property with
    // a pending promise that needs to be resolved. If an error was provided, we
    // will not resolve it with an error, since this is rendered at the root of
    // the app. We want the segment to error, not the entire app.
    const head = cacheNode.head;
    if (isDeferredRsc(head)) {
        head.resolve(null, debugInfo);
    }
}
const DEFERRED = Symbol();
// This type exists to distinguish a DeferredRsc from a Flight promise. It's a
// compromise to avoid adding an extra field on every Cache Node, which would be
// awkward because the pre-PPR parts of codebase would need to account for it,
// too. We can remove it once type Cache Node type is more settled.
export function isDeferredRsc(value) {
    return value && typeof value === 'object' && value.tag === DEFERRED;
}
function createDeferredRsc() {
    // Create an unresolved promise that represents data derived from a Flight
    // response. The promise will be resolved later as soon as we start receiving
    // data from the server, i.e. as soon as the Flight client decodes and returns
    // the top-level response object.
    // The `_debugInfo` field contains profiling information. Promises that are
    // created by Flight already have this info added by React; for any derived
    // promise created by the router, we need to transfer the Flight debug info
    // onto the derived promise.
    //
    // The debug info represents the latency between the start of the navigation
    // and the start of rendering. (It does not represent the time it takes for
    // whole stream to finish.)
    const debugInfo = [];
    let resolve;
    let reject;
    const pendingRsc = new Promise((res, rej)=>{
        resolve = res;
        reject = rej;
    });
    pendingRsc.status = 'pending';
    pendingRsc.resolve = (value, responseDebugInfo)=>{
        if (pendingRsc.status === 'pending') {
            const fulfilledRsc = pendingRsc;
            fulfilledRsc.status = 'fulfilled';
            fulfilledRsc.value = value;
            if (responseDebugInfo !== null) {
                // Transfer the debug info to the derived promise.
                debugInfo.push.apply(debugInfo, responseDebugInfo);
            }
            resolve(value);
        }
    };
    pendingRsc.reject = (error, responseDebugInfo)=>{
        if (pendingRsc.status === 'pending') {
            const rejectedRsc = pendingRsc;
            rejectedRsc.status = 'rejected';
            rejectedRsc.reason = error;
            if (responseDebugInfo !== null) {
                // Transfer the debug info to the derived promise.
                debugInfo.push.apply(debugInfo, responseDebugInfo);
            }
            reject(error);
        }
    };
    pendingRsc.tag = DEFERRED;
    pendingRsc._debugInfo = debugInfo;
    return pendingRsc;
}

//# sourceMappingURL=ppr-navigations.js.map