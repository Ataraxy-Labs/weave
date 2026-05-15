import { type SegmentParam } from '../utils/get-segment-param';
import { type InterceptionMarker } from '../utils/interception-routes';
export type RouteGroupAppRouteSegment = {
    type: 'route-group';
    name: string;
    /**
     * If present, this segment has an interception marker prefixing it.
     */
    interceptionMarker?: InterceptionMarker;
};
export type ParallelRouteAppRouteSegment = {
    type: 'parallel-route';
    name: string;
    /**
     * If present, this segment has an interception marker prefixing it.
     */
    interceptionMarker?: InterceptionMarker;
};
export type StaticAppRouteSegment = {
    type: 'static';
    name: string;
    /**
     * If present, this segment has an interception marker prefixing it.
     */
    interceptionMarker?: InterceptionMarker;
};
export type DynamicAppRouteSegment = {
    type: 'dynamic';
    name: string;
    param: SegmentParam;
    /**
     * If present, this segment has an interception marker prefixing it.
     */
    interceptionMarker?: InterceptionMarker;
};
/**
 * Represents a single segment in a route path.
 * Can be either static (e.g., "blog") or dynamic (e.g., "[slug]").
 */
export type AppRouteSegment = StaticAppRouteSegment | DynamicAppRouteSegment | RouteGroupAppRouteSegment | ParallelRouteAppRouteSegment;
export type NormalizedAppRouteSegment = StaticAppRouteSegment | DynamicAppRouteSegment;
export declare function parseAppRouteSegment(segment: string): AppRouteSegment | null;
export type AppRoute = {
    normalized: boolean;
    pathname: string;
    segments: AppRouteSegment[];
    dynamicSegments: DynamicAppRouteSegment[];
    interceptionMarker: InterceptionMarker | undefined;
    interceptingRoute: AppRoute | undefined;
    interceptedRoute: AppRoute | undefined;
};
export type NormalizedAppRoute = Omit<AppRoute, 'normalized' | 'segments'> & {
    normalized: true;
    segments: NormalizedAppRouteSegment[];
};
export declare function isNormalizedAppRoute(route: InterceptionAppRoute): route is NormalizedInterceptionAppRoute;
export type InterceptionAppRoute = Omit<AppRoute, 'interceptionMarker' | 'interceptingRoute' | 'interceptedRoute'> & {
    interceptionMarker: InterceptionMarker;
    interceptingRoute: AppRoute;
    interceptedRoute: AppRoute;
};
export type NormalizedInterceptionAppRoute = Omit<InterceptionAppRoute, 'normalized' | 'segments' | 'interceptionMarker' | 'interceptingRoute' | 'interceptedRoute'> & {
    normalized: true;
    segments: NormalizedAppRouteSegment[];
    interceptionMarker: InterceptionMarker;
    interceptingRoute: NormalizedAppRoute;
    interceptedRoute: NormalizedAppRoute;
};
export declare function isInterceptionAppRoute(route: NormalizedAppRoute): route is NormalizedInterceptionAppRoute;
export declare function parseAppRoute(pathname: string, normalized: true): NormalizedAppRoute;
export declare function parseAppRoute(pathname: string, normalized: false): AppRoute;
