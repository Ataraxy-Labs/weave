import type { MiddlewareManifest } from '../../../build/webpack/plugins/middleware-plugin';
import type { SetupOpts } from '../../../server/lib/router-utils/setup-dev-bundler';
import { type EntryKey } from './entry-key';
import type { CustomRoutes } from '../../../lib/load-custom-routes';
import type { Entrypoints } from '../../../build/swc/types';
interface InstrumentationDefinition {
    files: string[];
    name: 'instrumentation';
}
type TurbopackMiddlewareManifest = MiddlewareManifest & {
    instrumentation?: InstrumentationDefinition;
};
export declare class TurbopackManifestLoader {
    private actionManifests;
    private appPathsManifests;
    private buildManifests;
    private clientBuildManifests;
    private fontManifests;
    private middlewareManifests;
    private pagesManifests;
    private webpackStats;
    private encryptionKey;
    private cachedInterceptionRewrites;
    private readonly distDir;
    private readonly buildId;
    constructor({ distDir, buildId, encryptionKey, }: {
        buildId: string;
        distDir: string;
        encryptionKey: string;
    });
    delete(key: EntryKey): void;
    loadActionManifest(pageName: string): void;
    private mergeActionManifests;
    private writeActionManifest;
    loadAppPathsManifest(pageName: string): void;
    private writeAppPathsManifest;
    private writeWebpackStats;
    loadBuildManifest(pageName: string, type?: 'app' | 'pages'): void;
    loadClientBuildManifest(pageName: string, type?: 'app' | 'pages'): void;
    loadWebpackStats(pageName: string, type?: 'app' | 'pages'): void;
    private mergeWebpackStats;
    private mergeBuildManifests;
    private mergeClientBuildManifests;
    private writeInterceptionRouteRewriteManifest;
    private writeBuildManifest;
    private writeClientBuildManifest;
    loadFontManifest(pageName: string, type?: 'app' | 'pages'): void;
    private mergeFontManifests;
    private writeNextFontManifest;
    /**
     * @returns If the manifest was written or not
     */
    loadMiddlewareManifest(pageName: string, type: 'pages' | 'app' | 'middleware' | 'instrumentation'): boolean;
    getMiddlewareManifest(key: EntryKey): TurbopackMiddlewareManifest | undefined;
    deleteMiddlewareManifest(key: EntryKey): void;
    private mergeMiddlewareManifests;
    private writeMiddlewareManifest;
    loadPagesManifest(pageName: string): void;
    private mergePagesManifests;
    private writePagesManifest;
    writeManifests({ devRewrites, productionRewrites, entrypoints, }: {
        devRewrites: SetupOpts['fsChecker']['rewrites'] | undefined;
        productionRewrites: CustomRoutes['rewrites'] | undefined;
        entrypoints: Entrypoints;
    }): void;
}
export {};
