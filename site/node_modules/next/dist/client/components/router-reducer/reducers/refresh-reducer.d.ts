import type { ReadonlyReducerState, ReducerState } from '../router-reducer-types';
import { FreshnessPolicy } from '../ppr-navigations';
export declare function refreshReducer(state: ReadonlyReducerState): ReducerState;
export declare function refreshDynamicData(state: ReadonlyReducerState, freshnessPolicy: FreshnessPolicy.RefreshAll | FreshnessPolicy.HMRRefresh): ReducerState;
