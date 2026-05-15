import type { FlightRouterState } from '../../shared/lib/app-router-types';
import type { Params } from '../../server/request/params';
import { type NavigationPromises } from '../../shared/lib/hooks-client-context.shared-runtime';
/**
 * Creates instrumented navigation promises for the root app-router.
 */
export declare function createRootNavigationPromises(tree: FlightRouterState, pathname: string, searchParams: URLSearchParams, pathParams: Params): NavigationPromises | null;
/**
 * Creates merged navigation promises for nested layouts.
 * Merges parent promises with layout-specific segment promises.
 */
export declare function createNestedLayoutNavigationPromises(tree: FlightRouterState, parentNavPromises: NavigationPromises | null): NavigationPromises | null;
