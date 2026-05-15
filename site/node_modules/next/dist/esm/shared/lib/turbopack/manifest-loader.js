import { APP_PATHS_MANIFEST, BUILD_MANIFEST, INTERCEPTION_ROUTE_REWRITE_MANIFEST, MIDDLEWARE_BUILD_MANIFEST, MIDDLEWARE_MANIFEST, NEXT_FONT_MANIFEST, PAGES_MANIFEST, SERVER_REFERENCE_MANIFEST, TURBOPACK_CLIENT_BUILD_MANIFEST, TURBOPACK_CLIENT_MIDDLEWARE_MANIFEST, WEBPACK_STATS } from '../constants';
import { join, posix } from 'path';
import { readFileSync } from 'fs';
import { deleteCache } from '../../../server/dev/require-cache';
import { writeFileAtomic } from '../../../lib/fs/write-atomic';
import { isInterceptionRouteRewrite } from '../../../lib/generate-interception-routes-rewrites';
import getAssetPathFromRoute from '../router/utils/get-asset-path-from-route';
import { getEntryKey } from './entry-key';
import { getSortedRoutes } from '../router/utils';
import { existsSync } from 'fs';
import { addMetadataIdToRoute, addRouteSuffix, removeRouteSuffix } from '../../../server/dev/turbopack-utils';
import { tryToParsePath } from '../../../lib/try-to-parse-path';
import { safePathToRegexp } from '../router/utils/route-match-utils';
import { normalizeRewritesForBuildManifest, srcEmptySsgManifest, processRoute, createEdgeRuntimeManifest } from '../../../build/webpack/plugins/build-manifest-plugin-utils';
const getManifestPath = (page, distDir, name, type, firstCall)=>{
    let manifestPath = posix.join(distDir, `server`, type, type === 'middleware' || type === 'instrumentation' ? '' : type === 'app' ? page : getAssetPathFromRoute(page), name);
    if (firstCall) {
        const isSitemapRoute = /[\\/]sitemap(.xml)?\/route$/.test(page);
        // Check the ambiguity of /sitemap and /sitemap.xml
        if (isSitemapRoute && !existsSync(manifestPath)) {
            manifestPath = getManifestPath(page.replace(/\/sitemap\/route$/, '/sitemap.xml/route'), distDir, name, type, false);
        }
        // existsSync is faster than using the async version
        if (!existsSync(manifestPath) && page.endsWith('/route')) {
            // TODO: Improve implementation of metadata routes, currently it requires this extra check for the variants of the files that can be written.
            let metadataPage = addRouteSuffix(addMetadataIdToRoute(removeRouteSuffix(page)));
            manifestPath = getManifestPath(metadataPage, distDir, name, type, false);
        }
    }
    return manifestPath;
};
function readPartialManifestContent(distDir, name, pageName, type = 'pages') {
    const page = pageName;
    const manifestPath = getManifestPath(page, distDir, name, type, true);
    return readFileSync(posix.join(manifestPath), 'utf-8');
}
/// Helper class that stores a map of manifests and tracks if they have changed
/// since the last time they were written to disk. This is used to avoid
/// unnecessary writes to disk.
class ManifestsMap {
    set(key, value) {
        if (this.rawMap.get(key) === value) return;
        this.changed = true;
        this.rawMap.set(key, value);
        this.map.set(key, JSON.parse(value));
    }
    delete(key) {
        if (this.map.has(key)) {
            this.changed = true;
            this.rawMap.delete(key);
            this.map.delete(key);
        }
    }
    get(key) {
        return this.map.get(key);
    }
    takeChanged(extraInvalidationKey) {
        let changed = this.changed;
        if (extraInvalidationKey !== undefined) {
            const stringified = JSON.stringify(extraInvalidationKey);
            if (this.extraInvalidationKey !== stringified) {
                this.extraInvalidationKey = stringified;
                changed = true;
            }
        }
        this.changed = false;
        return changed;
    }
    values() {
        return this.map.values();
    }
    constructor(){
        this.rawMap = new Map();
        this.map = new Map();
        this.extraInvalidationKey = undefined;
        this.changed = true;
    }
}
export class TurbopackManifestLoader {
    constructor({ distDir, buildId, encryptionKey }){
        this.actionManifests = new ManifestsMap();
        this.appPathsManifests = new ManifestsMap();
        this.buildManifests = new ManifestsMap();
        this.clientBuildManifests = new ManifestsMap();
        this.fontManifests = new ManifestsMap();
        this.middlewareManifests = new ManifestsMap();
        this.pagesManifests = new ManifestsMap();
        this.webpackStats = new ManifestsMap();
        /// interceptionRewrites that have been written to disk
        /// This is used to avoid unnecessary writes if the rewrites haven't changed
        this.cachedInterceptionRewrites = undefined;
        this.distDir = distDir;
        this.buildId = buildId;
        this.encryptionKey = encryptionKey;
    }
    delete(key) {
        this.actionManifests.delete(key);
        this.appPathsManifests.delete(key);
        this.buildManifests.delete(key);
        this.clientBuildManifests.delete(key);
        this.fontManifests.delete(key);
        this.middlewareManifests.delete(key);
        this.pagesManifests.delete(key);
        this.webpackStats.delete(key);
    }
    loadActionManifest(pageName) {
        this.actionManifests.set(getEntryKey('app', 'server', pageName), readPartialManifestContent(this.distDir, `${SERVER_REFERENCE_MANIFEST}.json`, pageName, 'app'));
    }
    mergeActionManifests(manifests) {
        const manifest = {
            node: {},
            edge: {},
            encryptionKey: this.encryptionKey
        };
        function mergeActionIds(actionEntries, other) {
            for(const key in other){
                const action = actionEntries[key] ??= {
                    workers: {},
                    layer: {}
                };
                action.filename = other[key].filename;
                action.exportedName = other[key].exportedName;
                Object.assign(action.workers, other[key].workers);
                Object.assign(action.layer, other[key].layer);
            }
        }
        for (const m of manifests){
            mergeActionIds(manifest.node, m.node);
            mergeActionIds(manifest.edge, m.edge);
        }
        for(const key in manifest.node){
            const entry = manifest.node[key];
            entry.workers = sortObjectByKey(entry.workers);
            entry.layer = sortObjectByKey(entry.layer);
        }
        for(const key in manifest.edge){
            const entry = manifest.edge[key];
            entry.workers = sortObjectByKey(entry.workers);
            entry.layer = sortObjectByKey(entry.layer);
        }
        return manifest;
    }
    writeActionManifest() {
        if (!this.actionManifests.takeChanged()) {
            return;
        }
        const actionManifest = this.mergeActionManifests(this.actionManifests.values());
        const actionManifestJsonPath = join(this.distDir, 'server', `${SERVER_REFERENCE_MANIFEST}.json`);
        const actionManifestJsPath = join(this.distDir, 'server', `${SERVER_REFERENCE_MANIFEST}.js`);
        const json = JSON.stringify(actionManifest, null, 2);
        deleteCache(actionManifestJsonPath);
        deleteCache(actionManifestJsPath);
        writeFileAtomic(actionManifestJsonPath, json);
        writeFileAtomic(actionManifestJsPath, `self.__RSC_SERVER_MANIFEST=${JSON.stringify(json)}`);
    }
    loadAppPathsManifest(pageName) {
        this.appPathsManifests.set(getEntryKey('app', 'server', pageName), readPartialManifestContent(this.distDir, APP_PATHS_MANIFEST, pageName, 'app'));
    }
    writeAppPathsManifest() {
        if (!this.appPathsManifests.takeChanged()) {
            return;
        }
        const appPathsManifest = this.mergePagesManifests(this.appPathsManifests.values());
        const appPathsManifestPath = join(this.distDir, 'server', APP_PATHS_MANIFEST);
        deleteCache(appPathsManifestPath);
        writeFileAtomic(appPathsManifestPath, JSON.stringify(appPathsManifest, null, 2));
    }
    writeWebpackStats() {
        if (!this.webpackStats.takeChanged()) {
            return;
        }
        const webpackStats = this.mergeWebpackStats(this.webpackStats.values());
        const path = join(this.distDir, 'server', WEBPACK_STATS);
        deleteCache(path);
        writeFileAtomic(path, JSON.stringify(webpackStats, null, 2));
    }
    loadBuildManifest(pageName, type = 'pages') {
        this.buildManifests.set(getEntryKey(type, 'server', pageName), readPartialManifestContent(this.distDir, BUILD_MANIFEST, pageName, type));
    }
    loadClientBuildManifest(pageName, type = 'pages') {
        this.clientBuildManifests.set(getEntryKey(type, 'server', pageName), readPartialManifestContent(this.distDir, TURBOPACK_CLIENT_BUILD_MANIFEST, pageName, type));
    }
    loadWebpackStats(pageName, type = 'pages') {
        this.webpackStats.set(getEntryKey(type, 'client', pageName), readPartialManifestContent(this.distDir, WEBPACK_STATS, pageName, type));
    }
    mergeWebpackStats(statsFiles) {
        const entrypoints = {};
        const assets = new Map();
        const chunks = new Map();
        const modules = new Map();
        for (const statsFile of statsFiles){
            if (statsFile.entrypoints) {
                for (const [k, v] of Object.entries(statsFile.entrypoints)){
                    if (!entrypoints[k]) {
                        entrypoints[k] = v;
                    }
                }
            }
            if (statsFile.assets) {
                for (const asset of statsFile.assets){
                    if (!assets.has(asset.name)) {
                        assets.set(asset.name, asset);
                    }
                }
            }
            if (statsFile.chunks) {
                for (const chunk of statsFile.chunks){
                    if (!chunks.has(chunk.id)) {
                        chunks.set(chunk.id, chunk);
                    }
                }
            }
            if (statsFile.modules) {
                for (const module of statsFile.modules){
                    const id = module.id;
                    if (id != null) {
                        // Merge the chunk list for the module. This can vary across endpoints.
                        const existing = modules.get(id);
                        if (existing == null) {
                            modules.set(id, module);
                        } else if (module.chunks != null && existing.chunks != null) {
                            for (const chunk of module.chunks){
                                if (!existing.chunks.includes(chunk)) {
                                    existing.chunks.push(chunk);
                                }
                            }
                        }
                    }
                }
            }
        }
        return {
            version: 'Turbopack',
            entrypoints,
            assets: [
                ...assets.values()
            ],
            chunks: [
                ...chunks.values()
            ],
            modules: [
                ...modules.values()
            ]
        };
    }
    mergeBuildManifests(manifests) {
        const manifest = {
            pages: {
                '/_app': []
            },
            // Something in next.js depends on these to exist even for app dir rendering
            devFiles: [],
            polyfillFiles: [],
            lowPriorityFiles: [
                `static/${this.buildId}/_ssgManifest.js`,
                `static/${this.buildId}/_buildManifest.js`
            ],
            rootMainFiles: []
        };
        for (const m of manifests){
            Object.assign(manifest.pages, m.pages);
            if (m.rootMainFiles.length) manifest.rootMainFiles = m.rootMainFiles;
            // polyfillFiles should always be the same, so we can overwrite instead of actually merging
            if (m.polyfillFiles.length) manifest.polyfillFiles = m.polyfillFiles;
        }
        manifest.pages = sortObjectByKey(manifest.pages);
        return manifest;
    }
    mergeClientBuildManifests(manifests, rewrites, sortedPageKeys) {
        const manifest = {
            __rewrites: rewrites,
            sortedPages: sortedPageKeys
        };
        for (const m of manifests){
            Object.assign(manifest, m);
        }
        return sortObjectByKey(manifest);
    }
    writeInterceptionRouteRewriteManifest(devRewrites, productionRewrites) {
        const rewrites = productionRewrites ?? {
            ...devRewrites,
            beforeFiles: (devRewrites?.beforeFiles ?? []).map(processRoute),
            afterFiles: (devRewrites?.afterFiles ?? []).map(processRoute),
            fallback: (devRewrites?.fallback ?? []).map(processRoute)
        };
        const interceptionRewrites = JSON.stringify(rewrites.beforeFiles.filter(isInterceptionRouteRewrite));
        if (this.cachedInterceptionRewrites === interceptionRewrites) {
            return;
        }
        this.cachedInterceptionRewrites = interceptionRewrites;
        const interceptionRewriteManifestPath = join(this.distDir, 'server', `${INTERCEPTION_ROUTE_REWRITE_MANIFEST}.js`);
        deleteCache(interceptionRewriteManifestPath);
        writeFileAtomic(interceptionRewriteManifestPath, `self.__INTERCEPTION_ROUTE_REWRITE_MANIFEST=${JSON.stringify(interceptionRewrites)};`);
    }
    writeBuildManifest() {
        if (!this.buildManifests.takeChanged()) {
            return;
        }
        const buildManifest = this.mergeBuildManifests(this.buildManifests.values());
        const buildManifestPath = join(this.distDir, BUILD_MANIFEST);
        const middlewareBuildManifestPath = join(this.distDir, 'server', `${MIDDLEWARE_BUILD_MANIFEST}.js`);
        deleteCache(buildManifestPath);
        deleteCache(middlewareBuildManifestPath);
        writeFileAtomic(buildManifestPath, JSON.stringify(buildManifest, null, 2));
        writeFileAtomic(middlewareBuildManifestPath, // we use globalThis here because middleware can be node
        // which doesn't have "self"
        createEdgeRuntimeManifest(buildManifest));
        // Write fallback build manifest
        const fallbackBuildManifest = this.mergeBuildManifests([
            this.buildManifests.get(getEntryKey('pages', 'server', '_app')),
            this.buildManifests.get(getEntryKey('pages', 'server', '_error'))
        ].filter(Boolean));
        const fallbackBuildManifestPath = join(this.distDir, `fallback-${BUILD_MANIFEST}`);
        deleteCache(fallbackBuildManifestPath);
        writeFileAtomic(fallbackBuildManifestPath, JSON.stringify(fallbackBuildManifest, null, 2));
    }
    writeClientBuildManifest(entrypoints, devRewrites, productionRewrites) {
        const rewrites = normalizeRewritesForBuildManifest(productionRewrites ?? {
            ...devRewrites,
            beforeFiles: (devRewrites?.beforeFiles ?? []).map(processRoute),
            afterFiles: (devRewrites?.afterFiles ?? []).map(processRoute),
            fallback: (devRewrites?.fallback ?? []).map(processRoute)
        });
        const pagesKeys = [
            ...entrypoints.page.keys()
        ];
        if (entrypoints.global.app) {
            pagesKeys.push('/_app');
        }
        if (entrypoints.global.error) {
            pagesKeys.push('/_error');
        }
        const sortedPageKeys = getSortedRoutes(pagesKeys);
        if (!this.clientBuildManifests.takeChanged({
            rewrites,
            sortedPageKeys
        })) {
            return;
        }
        const clientBuildManifest = this.mergeClientBuildManifests(this.clientBuildManifests.values(), rewrites, sortedPageKeys);
        const clientBuildManifestJs = `self.__BUILD_MANIFEST = ${JSON.stringify(clientBuildManifest, null, 2)};self.__BUILD_MANIFEST_CB && self.__BUILD_MANIFEST_CB()`;
        writeFileAtomic(join(this.distDir, 'static', this.buildId, '_buildManifest.js'), clientBuildManifestJs);
        writeFileAtomic(join(this.distDir, 'static', this.buildId, '_ssgManifest.js'), srcEmptySsgManifest);
    }
    loadFontManifest(pageName, type = 'pages') {
        this.fontManifests.set(getEntryKey(type, 'server', pageName), readPartialManifestContent(this.distDir, `${NEXT_FONT_MANIFEST}.json`, pageName, type));
    }
    mergeFontManifests(manifests) {
        const manifest = {
            app: {},
            appUsingSizeAdjust: false,
            pages: {},
            pagesUsingSizeAdjust: false
        };
        for (const m of manifests){
            Object.assign(manifest.app, m.app);
            Object.assign(manifest.pages, m.pages);
            manifest.appUsingSizeAdjust = manifest.appUsingSizeAdjust || m.appUsingSizeAdjust;
            manifest.pagesUsingSizeAdjust = manifest.pagesUsingSizeAdjust || m.pagesUsingSizeAdjust;
        }
        manifest.app = sortObjectByKey(manifest.app);
        manifest.pages = sortObjectByKey(manifest.pages);
        return manifest;
    }
    async writeNextFontManifest() {
        if (!this.fontManifests.takeChanged()) {
            return;
        }
        const fontManifest = this.mergeFontManifests(this.fontManifests.values());
        const json = JSON.stringify(fontManifest, null, 2);
        const fontManifestJsonPath = join(this.distDir, 'server', `${NEXT_FONT_MANIFEST}.json`);
        const fontManifestJsPath = join(this.distDir, 'server', `${NEXT_FONT_MANIFEST}.js`);
        deleteCache(fontManifestJsonPath);
        deleteCache(fontManifestJsPath);
        writeFileAtomic(fontManifestJsonPath, json);
        writeFileAtomic(fontManifestJsPath, `self.__NEXT_FONT_MANIFEST=${JSON.stringify(json)}`);
    }
    /**
   * @returns If the manifest was written or not
   */ loadMiddlewareManifest(pageName, type) {
        const middlewareManifestPath = getManifestPath(pageName, this.distDir, MIDDLEWARE_MANIFEST, type, true);
        // middlewareManifest is actually "edge manifest" and not all routes are edge runtime. If it is not written we skip it.
        if (!existsSync(middlewareManifestPath)) {
            return false;
        }
        this.middlewareManifests.set(getEntryKey(type === 'middleware' || type === 'instrumentation' ? 'root' : type, 'server', pageName), readPartialManifestContent(this.distDir, MIDDLEWARE_MANIFEST, pageName, type));
        return true;
    }
    getMiddlewareManifest(key) {
        return this.middlewareManifests.get(key);
    }
    deleteMiddlewareManifest(key) {
        return this.middlewareManifests.delete(key);
    }
    mergeMiddlewareManifests(manifests) {
        const manifest = {
            version: 3,
            middleware: {},
            sortedMiddleware: [],
            functions: {}
        };
        let instrumentation = undefined;
        for (const m of manifests){
            Object.assign(manifest.functions, m.functions);
            Object.assign(manifest.middleware, m.middleware);
            if (m.instrumentation) {
                instrumentation = m.instrumentation;
            }
        }
        manifest.functions = sortObjectByKey(manifest.functions);
        manifest.middleware = sortObjectByKey(manifest.middleware);
        const updateFunctionDefinition = (fun)=>{
            return {
                ...fun,
                files: [
                    ...instrumentation?.files ?? [],
                    ...fun.files
                ]
            };
        };
        for (const key of Object.keys(manifest.middleware)){
            const value = manifest.middleware[key];
            manifest.middleware[key] = updateFunctionDefinition(value);
        }
        for (const key of Object.keys(manifest.functions)){
            const value = manifest.functions[key];
            manifest.functions[key] = updateFunctionDefinition(value);
        }
        for (const fun of Object.values(manifest.functions).concat(Object.values(manifest.middleware))){
            for (const matcher of fun.matchers){
                if (!matcher.regexp) {
                    matcher.regexp = safePathToRegexp(matcher.originalSource, [], {
                        delimiter: '/',
                        sensitive: false,
                        strict: true
                    }).source.replaceAll('\\/', '/');
                }
            }
        }
        manifest.sortedMiddleware = Object.keys(manifest.middleware);
        return manifest;
    }
    writeMiddlewareManifest() {
        if (!this.middlewareManifests.takeChanged()) {
            return;
        }
        const middlewareManifest = this.mergeMiddlewareManifests(this.middlewareManifests.values());
        // Server middleware manifest
        // Normalize regexes as it uses path-to-regexp
        for(const key in middlewareManifest.middleware){
            middlewareManifest.middleware[key].matchers.forEach((matcher)=>{
                if (!matcher.regexp.startsWith('^')) {
                    const parsedPage = tryToParsePath(matcher.regexp);
                    if (parsedPage.error || !parsedPage.regexStr) {
                        throw Object.defineProperty(new Error(`Invalid source: ${matcher.regexp}`), "__NEXT_ERROR_CODE", {
                            value: "E442",
                            enumerable: false,
                            configurable: true
                        });
                    }
                    matcher.regexp = parsedPage.regexStr;
                }
            });
        }
        const middlewareManifestPath = join(this.distDir, 'server', MIDDLEWARE_MANIFEST);
        deleteCache(middlewareManifestPath);
        writeFileAtomic(middlewareManifestPath, JSON.stringify(middlewareManifest, null, 2));
        // Client middleware manifest
        const matchers = middlewareManifest?.middleware['/']?.matchers || [];
        const clientMiddlewareManifestPath = join(this.distDir, 'static', this.buildId, `${TURBOPACK_CLIENT_MIDDLEWARE_MANIFEST}`);
        deleteCache(clientMiddlewareManifestPath);
        writeFileAtomic(clientMiddlewareManifestPath, JSON.stringify(matchers, null, 2));
    }
    loadPagesManifest(pageName) {
        this.pagesManifests.set(getEntryKey('pages', 'server', pageName), readPartialManifestContent(this.distDir, PAGES_MANIFEST, pageName));
    }
    mergePagesManifests(manifests) {
        const manifest = {};
        for (const m of manifests){
            Object.assign(manifest, m);
        }
        return sortObjectByKey(manifest);
    }
    writePagesManifest() {
        if (!this.pagesManifests.takeChanged()) {
            return;
        }
        const pagesManifest = this.mergePagesManifests(this.pagesManifests.values());
        const pagesManifestPath = join(this.distDir, 'server', PAGES_MANIFEST);
        deleteCache(pagesManifestPath);
        writeFileAtomic(pagesManifestPath, JSON.stringify(pagesManifest, null, 2));
    }
    writeManifests({ devRewrites, productionRewrites, entrypoints }) {
        this.writeActionManifest();
        this.writeAppPathsManifest();
        this.writeBuildManifest();
        this.writeInterceptionRouteRewriteManifest(devRewrites, productionRewrites);
        this.writeClientBuildManifest(entrypoints, devRewrites, productionRewrites);
        this.writeMiddlewareManifest();
        this.writeNextFontManifest();
        this.writePagesManifest();
        if (process.env.TURBOPACK_STATS != null) {
            this.writeWebpackStats();
        }
    }
}
function sortObjectByKey(obj) {
    return Object.keys(obj).sort().reduce((acc, key)=>{
        acc[key] = obj[key];
        return acc;
    }, {});
}

//# sourceMappingURL=manifest-loader.js.map