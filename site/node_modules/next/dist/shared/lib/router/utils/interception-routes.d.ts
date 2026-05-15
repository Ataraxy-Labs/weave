export declare const INTERCEPTION_ROUTE_MARKERS: readonly ["(..)(..)", "(.)", "(..)", "(...)"];
export type InterceptionMarker = (typeof INTERCEPTION_ROUTE_MARKERS)[number];
export declare function isInterceptionRouteAppPath(path: string): boolean;
type InterceptionRouteInformation = {
    /**
     * The intercepting route. This is the route that is being intercepted or the
     * route that the user was coming from. This is matched by the Next-Url
     * header.
     */
    interceptingRoute: string;
    /**
     * The intercepted route. This is the route that is being intercepted or the
     * route that the user is going to. This is matched by the request pathname.
     */
    interceptedRoute: string;
};
export declare function extractInterceptionRouteInformation(path: string): InterceptionRouteInformation;
export {};
