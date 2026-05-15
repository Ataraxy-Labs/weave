import type { CustomRoutes, Rewrite } from '../../../lib/load-custom-routes';
import type { BuildManifest } from '../../../server/get-page-files';
export type ClientBuildManifest = {
    [key: string]: string[];
};
export declare const srcEmptySsgManifest = "self.__SSG_MANIFEST=new Set;self.__SSG_MANIFEST_CB&&self.__SSG_MANIFEST_CB()";
export declare const processRoute: (r: Rewrite) => {
    source: string;
    destination: string;
    basePath?: false;
    locale?: false;
    has?: import("../../../lib/load-custom-routes").RouteHas[];
    missing?: import("../../../lib/load-custom-routes").RouteHas[];
    internal?: boolean;
    regex?: string;
};
export declare function normalizeRewritesForBuildManifest(rewrites: CustomRoutes['rewrites']): CustomRoutes['rewrites'];
export declare function createEdgeRuntimeManifest(originAssetMap: Partial<BuildManifest>): string;
