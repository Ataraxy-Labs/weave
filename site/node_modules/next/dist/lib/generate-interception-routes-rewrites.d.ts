import type { Rewrite } from './load-custom-routes';
import type { DeepReadonly } from '../shared/lib/deep-readonly';
export declare function generateInterceptionRoutesRewrites(appPaths: string[], basePath?: string): Rewrite[];
export declare function isInterceptionRouteRewrite(route: DeepReadonly<Rewrite>): boolean;
