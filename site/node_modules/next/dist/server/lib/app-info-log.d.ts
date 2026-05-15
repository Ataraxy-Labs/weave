import { type ConfiguredExperimentalFeature } from '../config';
export declare function logStartInfo({ networkUrl, appUrl, envInfo, experimentalFeatures, logBundler, cacheComponents, }: {
    networkUrl: string | null;
    appUrl: string | null;
    envInfo?: string[];
    experimentalFeatures?: ConfiguredExperimentalFeature[];
    logBundler: boolean;
    cacheComponents?: boolean;
}): void;
export declare function getStartServerInfo({ dir, dev, debugPrerender, }: {
    dir: string;
    dev: boolean;
    debugPrerender?: boolean;
}): Promise<{
    envInfo?: string[];
    experimentalFeatures?: ConfiguredExperimentalFeature[];
    cacheComponents?: boolean;
}>;
