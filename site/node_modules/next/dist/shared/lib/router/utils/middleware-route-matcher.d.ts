import type { BaseNextRequest } from '../../../../server/base-http';
import type { ProxyMatcher } from '../../../../build/analysis/get-page-static-info';
import type { Params } from '../../../../server/request/params';
export interface MiddlewareRouteMatch {
    (pathname: string | null | undefined, request: BaseNextRequest, query: Params): boolean;
}
export declare function getMiddlewareRouteMatcher(matchers: ProxyMatcher[]): MiddlewareRouteMatch;
