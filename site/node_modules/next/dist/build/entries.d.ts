import type { NextConfigComplete } from '../server/config-shared';
import type { webpack } from 'next/dist/compiled/webpack/webpack';
import type { ProxyConfig } from './analysis/get-page-static-info';
import type { LoadedEnvFiles } from '@next/env';
import type { AppLoaderOptions } from './webpack/loaders/next-app-loader';
import type { CompilerNameValues } from '../shared/lib/constants';
import type { __ApiPreviewProps } from '../server/api-utils';
import type { ServerRuntime } from '../types';
import type { PageExtensions } from './page-extensions-type';
import type { MappedPages } from './build-context';
import { PAGE_TYPES } from '../lib/page-types';
import type { createValidFileMatcher } from '../server/lib/find-page-file';
/**
 * Collect app pages, layouts, and default files from the app directory
 * @param appDir - The app directory path
 * @param validFileMatcher - File matcher object
 * @returns Object containing appPaths, layoutPaths, and defaultPaths arrays
 */
export declare function collectAppFiles(appDir: string, validFileMatcher: ReturnType<typeof createValidFileMatcher>): Promise<{
    appPaths: string[];
    layoutPaths: string[];
    defaultPaths: string[];
}>;
/**
 * Collect pages from the pages directory
 * @param pagesDir - The pages directory path
 * @param validFileMatcher - File matcher object
 * @returns Array of page file paths
 */
export declare function collectPagesFiles(pagesDir: string, validFileMatcher: ReturnType<typeof createValidFileMatcher>): Promise<string[]>;
export type RouteInfo = {
    route: string;
    filePath: string;
};
export type SlotInfo = {
    name: string;
    parent: string;
};
/**
 * Create a relative file path from a mapped page path
 * @param baseDir - The base directory path
 * @param filePath - The mapped file path (with private prefix)
 * @param prefix - The directory prefix ('pages' or 'app')
 * @param isSrcDir - Whether the project uses src directory structure
 * @returns The relative file path
 */
export declare function createRelativeFilePath(baseDir: string, filePath: string, prefix: 'pages' | 'app', isSrcDir: boolean): string;
/**
 * Process pages routes from mapped pages
 * @param mappedPages - The mapped pages object
 * @param baseDir - The base directory path
 * @param isSrcDir - Whether the project uses src directory structure
 * @returns Object containing pageRoutes and pageApiRoutes
 */
export declare function processPageRoutes(mappedPages: {
    [page: string]: string;
}, baseDir: string, isSrcDir: boolean): {
    pageRoutes: RouteInfo[];
    pageApiRoutes: RouteInfo[];
};
/**
 * Extract slots from app routes
 * @param mappedAppPages - The mapped app pages object
 * @returns Array of slot information
 */
export declare function extractSlotsFromAppRoutes(mappedAppPages: {
    [page: string]: string;
}): SlotInfo[];
/**
 * Extract slots from default files
 * @param mappedDefaultFiles - The mapped default files object
 * @returns Array of slot information
 */
export declare function extractSlotsFromDefaultFiles(mappedDefaultFiles: {
    [page: string]: string;
}): SlotInfo[];
/**
 * Combine and deduplicate slot arrays using a Set
 * @param slotArrays - Arrays of slot information to combine
 * @returns Deduplicated array of slots
 */
export declare function combineSlots(...slotArrays: SlotInfo[][]): SlotInfo[];
/**
 * Process app routes from mapped app pages
 * @param mappedAppPages - The mapped app pages object
 * @param validFileMatcher - File matcher object
 * @param baseDir - The base directory path
 * @param isSrcDir - Whether the project uses src directory structure
 * @returns Array of route information
 */
export declare function processAppRoutes(mappedAppPages: {
    [page: string]: string;
}, validFileMatcher: ReturnType<typeof createValidFileMatcher>, baseDir: string, isSrcDir: boolean): {
    appRoutes: RouteInfo[];
    appRouteHandlers: RouteInfo[];
};
/**
 * Process layout routes from mapped app layouts
 * @param mappedAppLayouts - The mapped app layouts object
 * @param baseDir - The base directory path
 * @param isSrcDir - Whether the project uses src directory structure
 * @returns Array of layout route information
 */
export declare function processLayoutRoutes(mappedAppLayouts: {
    [page: string]: string;
}, baseDir: string, isSrcDir: boolean): RouteInfo[];
type ObjectValue<T> = T extends {
    [key: string]: infer V;
} ? V : never;
/**
 * For a given page path removes the provided extensions.
 */
export declare function getPageFromPath(pagePath: string, pageExtensions: PageExtensions): string;
export declare function getPageFilePath({ absolutePagePath, pagesDir, appDir, rootDir, }: {
    absolutePagePath: string;
    pagesDir: string | undefined;
    appDir: string | undefined;
    rootDir: string;
}): string;
/**
 * Creates a mapping of route to page file path for a given list of page paths.
 * For example ['/middleware.ts'] is turned into  { '/middleware': `${ROOT_DIR_ALIAS}/middleware.ts` }
 */
export declare function createPagesMapping({ isDev, pageExtensions, pagePaths, pagesType, pagesDir, appDir, appDirOnly, }: {
    isDev: boolean;
    pageExtensions: PageExtensions;
    pagePaths: string[];
    pagesType: PAGE_TYPES;
    pagesDir: string | undefined;
    appDir: string | undefined;
    appDirOnly: boolean;
}): Promise<MappedPages>;
export interface CreateEntrypointsParams {
    buildId: string;
    config: NextConfigComplete;
    envFiles: LoadedEnvFiles;
    isDev: boolean;
    pages: MappedPages;
    pagesDir?: string;
    previewMode: __ApiPreviewProps;
    rootDir: string;
    rootPaths?: MappedPages;
    appDir?: string;
    appPaths?: MappedPages;
    pageExtensions: PageExtensions;
    hasInstrumentationHook?: boolean;
}
export declare function getEdgeServerEntry(opts: {
    rootDir: string;
    absolutePagePath: string;
    buildId: string;
    bundlePath: string;
    config: NextConfigComplete;
    isDev: boolean;
    isServerComponent: boolean;
    page: string;
    pages: MappedPages;
    middleware?: Partial<ProxyConfig>;
    pagesType: PAGE_TYPES;
    appDirLoader?: string;
    hasInstrumentationHook?: boolean;
    preferredRegion: string | string[] | undefined;
    middlewareConfig?: ProxyConfig;
}): {
    import: string;
    layer: "rsc";
    filename?: undefined;
} | {
    import: string;
    layer: "middleware";
    filename: string | undefined;
} | {
    import: string;
    layer: "api-edge";
    filename?: undefined;
} | {
    import: string;
    layer: "ssr" | undefined;
    filename?: undefined;
};
export declare function getInstrumentationEntry(opts: {
    absolutePagePath: string;
    isEdgeServer: boolean;
    isDev: boolean;
}): {
    import: string;
    filename: string;
    layer: "instrument";
};
export declare function getAppLoader(): "builtin:next-app-loader" | "next-app-loader";
export declare function getAppEntry(opts: Readonly<AppLoaderOptions>): {
    import: string;
    layer: "rsc";
};
export declare function getClientEntry(opts: {
    absolutePagePath: string;
    page: string;
}): string | string[];
export declare function runDependingOnPageType<T>(params: {
    onClient: () => T;
    onEdgeServer: () => T;
    onServer: () => T;
    page: string;
    pageRuntime: ServerRuntime;
    pageType?: PAGE_TYPES;
}): void;
export declare function createEntrypoints(params: CreateEntrypointsParams): Promise<{
    client: webpack.EntryObject;
    server: webpack.EntryObject;
    edgeServer: webpack.EntryObject;
    middlewareMatchers: undefined;
}>;
export declare function finalizeEntrypoint({ name, compilerType, value, isServerComponent, hasAppDir, }: {
    compilerType: CompilerNameValues;
    name: string;
    value: ObjectValue<webpack.EntryObject>;
    isServerComponent?: boolean;
    hasAppDir?: boolean;
}): ObjectValue<webpack.EntryObject>;
export {};
