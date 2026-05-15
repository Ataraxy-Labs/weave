import type { LoaderTree } from '../lib/app-dir-module';
import { type FlightRouterState } from '../../shared/lib/app-router-types';
import type { GetDynamicParamFromSegment } from './app-render';
export declare function createFlightRouterStateFromLoaderTree(loaderTree: LoaderTree, getDynamicParamFromSegment: GetDynamicParamFromSegment, searchParams: any): FlightRouterState;
export declare function createRouteTreePrefetch(loaderTree: LoaderTree, getDynamicParamFromSegment: GetDynamicParamFromSegment): FlightRouterState;
