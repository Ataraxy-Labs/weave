import type { DynamicParamTypesShort, LoadingModuleData } from '../../shared/lib/app-router-types';
import type { ManifestNode } from '../../build/webpack/plugins/flight-manifest-plugin';
import { type SegmentRequestKey } from '../../shared/lib/segment-cache/segment-value-encoding';
export type RootTreePrefetch = {
    buildId: string;
    tree: TreePrefetch;
    staleTime: number;
};
export type TreePrefetch = {
    name: string;
    paramType: DynamicParamTypesShort | null;
    paramKey: string | null;
    slots: null | {
        [parallelRouteKey: string]: TreePrefetch;
    };
    /** Whether this segment should be fetched using a runtime prefetch */
    hasRuntimePrefetch: boolean;
    isRootLayout: boolean;
};
export type SegmentPrefetch = {
    buildId: string;
    rsc: React.ReactNode | null;
    loading: LoadingModuleData | Promise<LoadingModuleData>;
    isPartial: boolean;
};
export declare function collectSegmentData(isCacheComponentsEnabled: boolean, fullPageDataBuffer: Buffer, staleTime: number, clientModules: ManifestNode, serverConsumerManifest: any): Promise<Map<SegmentRequestKey, Buffer>>;
