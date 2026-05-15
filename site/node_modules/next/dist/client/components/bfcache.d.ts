import type { FlightRouterState } from '../../shared/lib/app-router-types';
export type RouterBFCacheEntry = {
    tree: FlightRouterState;
    stateKey: string;
    next: RouterBFCacheEntry | null;
};
/**
 * Keeps track of the most recent N trees (FlightRouterStates) that were active
 * at a certain segment level. E.g. for a segment "/a/b/[param]", this hook
 * tracks the last N param values that the router rendered for N.
 *
 * The result of this hook precisely determines the number and order of
 * trees that are rendered in parallel at their segment level.
 *
 * The purpose of this cache is to we can preserve the React and DOM state of
 * some number of inactive trees, by rendering them in an <Activity> boundary.
 * That means it would not make sense for the the lifetime of the cache to be
 * any longer than the lifetime of the React tree; e.g. if the hook were
 * unmounted, then the React tree would be, too. So, we use React state to
 * manage it.
 *
 * Note that we don't store the RSC data for the cache entries in this hook —
 * the data for inactive segments is stored in the parent CacheNode, which
 * *does* have a longer lifetime than the React tree. This hook only determines
 * which of those trees should have their *state* preserved, by <Activity>.
 */
export declare function useRouterBFCache(activeTree: FlightRouterState, activeStateKey: string): RouterBFCacheEntry;
