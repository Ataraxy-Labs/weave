export declare function getRevalidateReason(params: {
    isOnDemandRevalidate?: boolean;
    isStaticGeneration?: boolean;
}): 'on-demand' | 'stale' | undefined;
