import { refreshDynamicData } from './refresh-reducer';
import { FreshnessPolicy } from '../ppr-navigations';
export function hmrRefreshReducer(state) {
    return refreshDynamicData(state, FreshnessPolicy.HMRRefresh);
}

//# sourceMappingURL=hmr-refresh-reducer.js.map