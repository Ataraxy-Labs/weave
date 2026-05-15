import type { FlightRouterState, FlightSegmentPath } from '../../../../shared/lib/app-router-types';
import type { Mutable, NavigateAction, ReadonlyReducerState, ReducerState } from '../router-reducer-types';
import { type NavigationResult } from '../../segment-cache/navigation';
export declare const DYNAMIC_STALETIME_MS: number;
export declare const STATIC_STALETIME_MS: number;
export declare function handleExternalUrl(state: ReadonlyReducerState, mutable: Mutable, url: string, pendingPush: boolean): ReducerState;
export declare function generateSegmentsFromPatch(flightRouterPatch: FlightRouterState): FlightSegmentPath[];
export declare function handleNavigationResult(url: URL, state: ReadonlyReducerState, mutable: Mutable, pendingPush: boolean, result: NavigationResult): ReducerState;
export declare function navigateReducer(state: ReadonlyReducerState, action: NavigateAction): ReducerState;
