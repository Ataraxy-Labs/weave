#!/usr/bin/env node
"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "nextTypegen", {
    enumerable: true,
    get: function() {
        return nextTypegen;
    }
});
const _fs = require("fs");
const _path = /*#__PURE__*/ _interop_require_wildcard(require("path"));
const _promises = require("fs/promises");
const _config = /*#__PURE__*/ _interop_require_default(require("../server/config"));
const _utils = require("../server/lib/utils");
const _constants = require("../shared/lib/constants");
const _getprojectdir = require("../lib/get-project-dir");
const _findpagesdir = require("../lib/find-pages-dir");
const _verifytypescriptsetup = require("../lib/verify-typescript-setup");
const _entries = require("../build/entries");
const _pagetypes = require("../lib/page-types");
const _routetypesutils = require("../server/lib/router-utils/route-types-utils");
const _cachelifetypeutils = require("../server/lib/router-utils/cache-life-type-utils");
const _findpagefile = require("../server/lib/find-page-file");
const _installbindings = require("../build/swc/install-bindings");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) {
        return obj;
    }
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") {
        return {
            default: obj
        };
    }
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) {
        return cache.get(obj);
    }
    var newObj = {
        __proto__: null
    };
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for(var key in obj){
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) {
                Object.defineProperty(newObj, key, desc);
            } else {
                newObj[key] = obj[key];
            }
        }
    }
    newObj.default = obj;
    if (cache) {
        cache.set(obj, newObj);
    }
    return newObj;
}
const nextTypegen = async (_options, directory)=>{
    var _nextConfig_experimental;
    const baseDir = (0, _getprojectdir.getProjectDir)(directory);
    // Check if the provided directory exists
    if (!(0, _fs.existsSync)(baseDir)) {
        (0, _utils.printAndExit)(`> No such directory exists as the project root: ${baseDir}`);
    }
    const nextConfig = await (0, _config.default)(_constants.PHASE_PRODUCTION_BUILD, baseDir);
    await (0, _installbindings.installBindings)((_nextConfig_experimental = nextConfig.experimental) == null ? void 0 : _nextConfig_experimental.useWasmBinary);
    const distDir = (0, _path.join)(baseDir, nextConfig.distDir);
    const { pagesDir, appDir } = (0, _findpagesdir.findPagesDir)(baseDir);
    await (0, _verifytypescriptsetup.verifyTypeScriptSetup)({
        dir: baseDir,
        distDir: nextConfig.distDir,
        typeCheckPreflight: false,
        tsconfigPath: nextConfig.typescript.tsconfigPath,
        disableStaticImages: nextConfig.images.disableStaticImages,
        hasAppDir: !!appDir,
        hasPagesDir: !!pagesDir,
        isolatedDevBuild: nextConfig.experimental.isolatedDevBuild,
        appDir: appDir || undefined,
        pagesDir: pagesDir || undefined
    });
    console.log('Generating route types...');
    const routeTypesFilePath = (0, _path.join)(distDir, 'types', 'routes.d.ts');
    const validatorFilePath = (0, _path.join)(distDir, 'types', 'validator.ts');
    await (0, _promises.mkdir)((0, _path.join)(distDir, 'types'), {
        recursive: true
    });
    let pageRoutes = [];
    let appRoutes = [];
    let appRouteHandlers = [];
    let layoutRoutes = [];
    let slots = [];
    let pageApiRoutes = [];
    let mappedPages = {};
    let mappedAppPages = {};
    let mappedAppLayouts = {};
    // Helper function to reduce createPagesMapping duplication
    const createMapping = (pagePaths, pagesType)=>(0, _entries.createPagesMapping)({
            pagePaths,
            isDev: false,
            pagesType,
            pageExtensions: nextConfig.pageExtensions,
            pagesDir,
            appDir,
            appDirOnly: !!appDir && !pagesDir
        });
    const validFileMatcher = (0, _findpagefile.createValidFileMatcher)(nextConfig.pageExtensions, appDir);
    const isSrcDir = _path.default.relative(baseDir, pagesDir || appDir || '').startsWith('src');
    // Build pages routes
    if (pagesDir) {
        const pagePaths = await (0, _entries.collectPagesFiles)(pagesDir, validFileMatcher);
        mappedPages = await createMapping(pagePaths, _pagetypes.PAGE_TYPES.PAGES);
        // Process pages routes
        const processedPages = (0, _entries.processPageRoutes)(mappedPages, baseDir, isSrcDir);
        pageRoutes = processedPages.pageRoutes;
        pageApiRoutes = processedPages.pageApiRoutes;
    }
    // Build app routes
    if (appDir) {
        // Collect app pages, layouts, and default files in a single directory traversal
        const { appPaths, layoutPaths, defaultPaths } = await (0, _entries.collectAppFiles)(appDir, validFileMatcher);
        mappedAppPages = await createMapping(appPaths, _pagetypes.PAGE_TYPES.APP);
        mappedAppLayouts = await createMapping(layoutPaths, _pagetypes.PAGE_TYPES.APP);
        const mappedDefaultFiles = await createMapping(defaultPaths, _pagetypes.PAGE_TYPES.APP);
        // Process app routes and extract slots from both pages and default files
        const slotsFromPages = (0, _entries.extractSlotsFromAppRoutes)(mappedAppPages);
        const slotsFromDefaults = (0, _entries.extractSlotsFromDefaultFiles)(mappedDefaultFiles);
        // Combine slots and deduplicate using Set
        slots = (0, _entries.combineSlots)(slotsFromPages, slotsFromDefaults);
        const result = (0, _entries.processAppRoutes)(mappedAppPages, validFileMatcher, baseDir, isSrcDir);
        appRoutes = result.appRoutes;
        appRouteHandlers = result.appRouteHandlers;
        // Process layout routes
        layoutRoutes = (0, _entries.processLayoutRoutes)(mappedAppLayouts, baseDir, isSrcDir);
    }
    const routeTypesManifest = await (0, _routetypesutils.createRouteTypesManifest)({
        dir: baseDir,
        pageRoutes,
        appRoutes,
        appRouteHandlers,
        pageApiRoutes,
        layoutRoutes,
        slots,
        redirects: nextConfig.redirects,
        rewrites: nextConfig.rewrites,
        validatorFilePath
    });
    await (0, _routetypesutils.writeRouteTypesManifest)(routeTypesManifest, routeTypesFilePath, nextConfig);
    await (0, _routetypesutils.writeValidatorFile)(routeTypesManifest, validatorFilePath);
    // Generate cache-life types if cacheLife config exists
    const cacheLifeFilePath = (0, _path.join)(distDir, 'types', 'cache-life.d.ts');
    (0, _cachelifetypeutils.writeCacheLifeTypes)(nextConfig.cacheLife, cacheLifeFilePath);
    console.log('✓ Types generated successfully');
};

//# sourceMappingURL=next-typegen.js.map