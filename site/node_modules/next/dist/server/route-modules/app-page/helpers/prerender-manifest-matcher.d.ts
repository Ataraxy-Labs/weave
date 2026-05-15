import type { DynamicPrerenderManifestRoute, PrerenderManifest } from '../../../../build';
import type { DeepReadonly } from '../../../../shared/lib/deep-readonly';
/**
 * A matcher for the prerender manifest.
 *
 * This class is used to match the pathname to the dynamic route.
 */
export declare class PrerenderManifestMatcher {
    private readonly matchers;
    constructor(pathname: string, prerenderManifest: DeepReadonly<PrerenderManifest>);
    /**
     * Match the pathname to the dynamic route. If no match is found, an error is
     * thrown.
     *
     * @param pathname - The pathname to match.
     * @returns The dynamic route that matches the pathname.
     */
    match(pathname: string): DeepReadonly<DynamicPrerenderManifestRoute> | null;
}
