import { UNDERSCORE_GLOBAL_ERROR_ROUTE, UNDERSCORE_NOT_FOUND_ROUTE } from '../../../../shared/lib/constants';
import { UNDERSCORE_GLOBAL_ERROR_ROUTE_ENTRY, UNDERSCORE_NOT_FOUND_ROUTE_ENTRY } from '../../../../shared/lib/entry-constants';
import path from 'path';
import { bold } from '../../../../lib/picocolors';
import { getModuleBuildInfo } from '../get-module-build-info';
import { verifyRootLayout } from '../../../../lib/verify-root-layout';
import * as Log from '../../../output/log';
import { APP_DIR_ALIAS } from '../../../../lib/constants';
import { createMetadataExportsCode, createStaticMetadataFromRoute } from '../metadata/discover';
import { promises as fs } from 'fs';
import { isAppRouteRoute } from '../../../../lib/is-app-route-route';
import { AppPathnameNormalizer } from '../../../../server/normalizers/built/app/app-pathname-normalizer';
import { isAppBuiltinPage } from '../../../utils';
import { loadEntrypoint } from '../../../load-entrypoint';
import { isGroupSegment, DEFAULT_SEGMENT_KEY, PAGE_SEGMENT_KEY } from '../../../../shared/lib/segment';
import { getFilesInDir } from '../../../../lib/get-files-in-dir';
import { PARALLEL_ROUTE_DEFAULT_PATH } from '../../../../client/components/builtin/default';
import { PARALLEL_ROUTE_DEFAULT_NULL_PATH } from '../../../../client/components/builtin/default-null';
import { createAppRouteCode } from './create-app-route-code';
import { MissingDefaultParallelRouteError } from '../../../../shared/lib/errors/missing-default-parallel-route-error';
import { isInterceptionRouteAppPath } from '../../../../shared/lib/router/utils/interception-routes';
import { normalizePathSep } from '../../../../shared/lib/page-path/normalize-path-sep';
import { installBindings } from '../../../swc/install-bindings';
const HTTP_ACCESS_FALLBACKS = {
    'not-found': 'not-found',
    forbidden: 'forbidden',
    unauthorized: 'unauthorized'
};
const defaultHTTPAccessFallbackPaths = {
    'not-found': 'next/dist/client/components/builtin/not-found.js',
    forbidden: 'next/dist/client/components/builtin/forbidden.js',
    unauthorized: 'next/dist/client/components/builtin/unauthorized.js'
};
const FILE_TYPES = {
    layout: 'layout',
    template: 'template',
    error: 'error',
    loading: 'loading',
    'global-error': 'global-error',
    'global-not-found': 'global-not-found',
    ...HTTP_ACCESS_FALLBACKS
};
const GLOBAL_ERROR_FILE_TYPE = 'global-error';
const GLOBAL_NOT_FOUND_FILE_TYPE = 'global-not-found';
const PAGE_SEGMENT = 'page$';
const PARALLEL_VIRTUAL_SEGMENT = 'slot$';
const defaultGlobalErrorPath = 'next/dist/client/components/builtin/global-error.js';
const defaultNotFoundPath = 'next/dist/client/components/builtin/not-found.js';
const defaultEmptyStubPath = 'next/dist/client/components/builtin/empty-stub.js';
const defaultLayoutPath = 'next/dist/client/components/builtin/layout.js';
const defaultGlobalNotFoundPath = 'next/dist/client/components/builtin/global-not-found.js';
const appErrorPath = 'next/dist/client/components/builtin/app-error.js';
const normalizeParallelKey = (key)=>key.startsWith('@') ? key.slice(1) : key;
const isCatchAllSegment = (segment)=>segment.startsWith('[...') || segment.startsWith('[[...');
const isDirectory = async (pathname)=>{
    try {
        const stat = await fs.stat(pathname);
        return stat.isDirectory();
    } catch (err) {
        return false;
    }
};
async function createTreeCodeFromPath(pagePath, { page, resolveDir, resolver, resolveParallelSegments, hasChildRoutesForSegment, metadataResolver, pageExtensions, basePath, collectedDeclarations, isGlobalNotFoundEnabled }) {
    const splittedPath = pagePath.split(/[\\/]/, 1);
    const isNotFoundRoute = page === UNDERSCORE_NOT_FOUND_ROUTE_ENTRY;
    const isAppErrorRoute = page === UNDERSCORE_GLOBAL_ERROR_ROUTE_ENTRY;
    const isDefaultNotFound = isAppBuiltinPage(pagePath);
    const appDirPrefix = isDefaultNotFound ? APP_DIR_ALIAS : splittedPath[0];
    let rootLayout;
    let globalError = defaultGlobalErrorPath;
    let globalNotFound = defaultNotFoundPath;
    async function resolveAdjacentParallelSegments(segmentPath) {
        const absoluteSegmentPath = resolveDir(`${appDirPrefix}${segmentPath}`);
        if (!absoluteSegmentPath) {
            return [];
        }
        const segmentIsDirectory = await isDirectory(absoluteSegmentPath);
        if (!segmentIsDirectory) {
            return [];
        }
        // We need to resolve all parallel routes in this level.
        const files = await fs.opendir(absoluteSegmentPath);
        const parallelSegments = [
            'children'
        ];
        for await (const dirent of files){
            // Make sure name starts with "@" and is a directory.
            if (dirent.isDirectory() && dirent.name.charCodeAt(0) === 64) {
                parallelSegments.push(dirent.name);
            }
        }
        return parallelSegments;
    }
    async function createSubtreePropsFromSegmentPath(segments, nestedCollectedDeclarations) {
        const segmentPath = segments.join('/');
        // Existing tree are the children of the current segment
        const props = {};
        // Root layer could be 1st layer of normal routes
        const isRootLayer = segments.length === 0;
        const isRootLayoutOrRootPage = segments.length <= 1;
        // We need to resolve all parallel routes in this level.
        const parallelSegments = [];
        if (isRootLayer) {
            parallelSegments.push([
                'children',
                ''
            ]);
        } else {
            parallelSegments.push(...resolveParallelSegments(segmentPath));
        }
        let metadata = null;
        const routerDirPath = `${appDirPrefix}${segmentPath}`;
        const resolvedRouteDir = resolveDir(routerDirPath);
        if (resolvedRouteDir && // Do not collect metadata for app-error route as it's for generating pure static 500.html
        !normalizePathSep(pagePath).endsWith(appErrorPath)) {
            metadata = await createStaticMetadataFromRoute(resolvedRouteDir, {
                basePath,
                segment: segmentPath,
                metadataResolver,
                isRootLayoutOrRootPage,
                pageExtensions
            });
        }
        for (const [parallelKey, parallelSegment] of parallelSegments){
            // if parallelSegment is the page segment (ie, `page$` and not ['page$']), it gets loaded into the __PAGE__ slot
            // as it's the page for the current route.
            if (parallelSegment === PAGE_SEGMENT) {
                const matchedPagePath = `${appDirPrefix}${segmentPath}${parallelKey === 'children' ? '' : `/${parallelKey}`}/page`;
                const resolvedPagePath = await resolver(matchedPagePath);
                if (resolvedPagePath) {
                    const varName = `page${nestedCollectedDeclarations.length}`;
                    nestedCollectedDeclarations.push([
                        varName,
                        resolvedPagePath
                    ]);
                    // Use '' for segment as it's the page. There can't be a segment called '' so this is the safest way to add it.
                    props[normalizeParallelKey(parallelKey)] = `['${PAGE_SEGMENT_KEY}', {}, {
          page: [${varName}, ${JSON.stringify(resolvedPagePath)}],
          ${createMetadataExportsCode(metadata)}
        }]`;
                    continue;
                } else {
                    throw Object.defineProperty(new Error(`Can't resolve ${matchedPagePath}`), "__NEXT_ERROR_CODE", {
                        value: "E711",
                        enumerable: false,
                        configurable: true
                    });
                }
            }
            // if the parallelSegment was not matched to the __PAGE__ slot, then it's a parallel route at this level.
            // the code below recursively traverses the parallel slots directory to match the corresponding __PAGE__ for each parallel slot
            // while also filling in layout/default/etc files into the loader tree at each segment level.
            const subSegmentPath = [
                ...segments
            ];
            if (parallelKey !== 'children') {
                // A `children` parallel key should have already been processed in the above segment
                // So we exclude it when constructing the subsegment path for the remaining segment levels
                subSegmentPath.push(parallelKey);
            }
            const normalizedParallelSegment = Array.isArray(parallelSegment) ? parallelSegment[0] : parallelSegment;
            if (normalizedParallelSegment !== PAGE_SEGMENT && normalizedParallelSegment !== PARALLEL_VIRTUAL_SEGMENT) {
                // If we don't have a page segment, nor a special $children marker, it means we need to traverse the next directory
                // (ie, `normalizedParallelSegment` would correspond with the folder that contains the next level of pages/layout/etc)
                // we push it to the subSegmentPath so that we can fill in the loader tree for that segment.
                subSegmentPath.push(normalizedParallelSegment);
            }
            const parallelSegmentPath = subSegmentPath.join('/');
            // Fill in the loader tree for all of the special files types (layout, default, etc) at this level
            // `page` is not included here as it's added above.
            const filePathEntries = await Promise.all(Object.values(FILE_TYPES).map(async (file)=>{
                return [
                    file,
                    await resolver(`${appDirPrefix}${// TODO-APP: parallelSegmentPath sometimes ends in `/` but sometimes it doesn't. This should be consistent.
                    parallelSegmentPath.endsWith('/') ? parallelSegmentPath : parallelSegmentPath + '/'}${file}`)
                ];
            }));
            const filePaths = new Map(filePathEntries);
            // Only resolve global-* convention files at the root layer
            if (isRootLayer) {
                const resolvedGlobalErrorPath = await resolver(`${appDirPrefix}/${GLOBAL_ERROR_FILE_TYPE}`);
                if (resolvedGlobalErrorPath) {
                    globalError = resolvedGlobalErrorPath;
                }
                // Add global-error to root layer's filePaths, so that it's always available,
                // by default it's the built-in global-error.js
                filePaths.set(GLOBAL_ERROR_FILE_TYPE, globalError);
                // TODO(global-not-found): remove this flag assertion condition
                //  once global-not-found is stable
                if (isGlobalNotFoundEnabled) {
                    const resolvedGlobalNotFoundPath = await resolver(`${appDirPrefix}/${GLOBAL_NOT_FOUND_FILE_TYPE}`);
                    if (resolvedGlobalNotFoundPath) {
                        globalNotFound = resolvedGlobalNotFoundPath;
                    }
                    // Add global-not-found to root layer's filePaths, so that it's always available,
                    // by default it's the built-in global-not-found.js
                    filePaths.set(GLOBAL_NOT_FOUND_FILE_TYPE, globalNotFound);
                }
            }
            let definedFilePaths = Array.from(filePaths.entries()).filter(([, filePath])=>filePath !== undefined);
            // Add default access fallback as root fallback if not present
            const existedConventionNames = new Set(definedFilePaths.map(([type])=>type));
            // If the first layer is a group route, we treat it as root layer
            const isFirstLayerGroupRoute = segments.length === 1 && subSegmentPath.filter((seg)=>isGroupSegment(seg)).length === 1;
            if (isRootLayer || isFirstLayerGroupRoute) {
                const accessFallbackTypes = Object.keys(defaultHTTPAccessFallbackPaths);
                for (const type of accessFallbackTypes){
                    const hasRootFallbackFile = await resolver(`${appDirPrefix}/${FILE_TYPES[type]}`);
                    const hasLayerFallbackFile = existedConventionNames.has(type);
                    // If you already have a root access error fallback, don't insert default access error boundary to group routes root
                    if (// Is treated as root layout and without boundary
                    !(hasRootFallbackFile && isFirstLayerGroupRoute) && // Does not have a fallback boundary file
                    !hasLayerFallbackFile) {
                        const defaultFallbackPath = defaultHTTPAccessFallbackPaths[type];
                        if (!(isDefaultNotFound && type === 'not-found')) {
                            definedFilePaths.push([
                                type,
                                defaultFallbackPath
                            ]);
                        }
                    }
                }
            }
            if (!rootLayout) {
                var _definedFilePaths_find;
                const layoutPath = (_definedFilePaths_find = definedFilePaths.find(([type])=>type === 'layout')) == null ? void 0 : _definedFilePaths_find[1];
                rootLayout = layoutPath;
                // When `global-not-found` is disabled, we insert a default layout if
                // root layout is presented. This logic and the default layout will be removed
                // once `global-not-found` is stabilized.
                if (!isGlobalNotFoundEnabled && isDefaultNotFound && !layoutPath && !rootLayout) {
                    rootLayout = defaultLayoutPath;
                    definedFilePaths.push([
                        'layout',
                        rootLayout
                    ]);
                }
            }
            let parallelSegmentKey = Array.isArray(parallelSegment) ? parallelSegment[0] : parallelSegment;
            // normalize the parallel segment key to remove any special markers that we inserted in the
            // earlier logic (such as children$ and page$). These should never appear in the loader tree, and
            // should instead be the corresponding segment keys (ie `__PAGE__`) or the `children` parallel route.
            parallelSegmentKey = parallelSegmentKey === PARALLEL_VIRTUAL_SEGMENT ? '(slot)' : parallelSegmentKey === PAGE_SEGMENT ? PAGE_SEGMENT_KEY : parallelSegmentKey;
            const normalizedParallelKey = normalizeParallelKey(parallelKey);
            let subtreeCode;
            // If it's root not found page, set not-found boundary as children page
            if (isNotFoundRoute) {
                if (normalizedParallelKey === 'children') {
                    var _definedFilePaths_find1;
                    const matchedGlobalNotFound = isGlobalNotFoundEnabled ? ((_definedFilePaths_find1 = definedFilePaths.find(([type])=>type === GLOBAL_NOT_FOUND_FILE_TYPE)) == null ? void 0 : _definedFilePaths_find1[1]) ?? defaultGlobalNotFoundPath : undefined;
                    // If custom global-not-found.js is defined, use global-not-found.js
                    if (matchedGlobalNotFound) {
                        const varName = `notFound${nestedCollectedDeclarations.length}`;
                        nestedCollectedDeclarations.push([
                            varName,
                            matchedGlobalNotFound
                        ]);
                        const layoutName = `layout${nestedCollectedDeclarations.length}`;
                        nestedCollectedDeclarations.push([
                            layoutName,
                            defaultEmptyStubPath
                        ]);
                        subtreeCode = `{
              children: [${JSON.stringify(UNDERSCORE_NOT_FOUND_ROUTE)}, {
                children: ['${PAGE_SEGMENT_KEY}', {}, {
                  layout: [
                    ${varName},
                    ${JSON.stringify(matchedGlobalNotFound)}
                  ],
                  page: [
                    ${layoutName},
                    ${JSON.stringify(defaultEmptyStubPath)}
                  ]
                }]
              }, {}]
            }`;
                    } else {
                        var _definedFilePaths_find2;
                        // If custom not-found.js is found, use it and layout to compose the page,
                        // and fallback to built-in not-found component if doesn't exist.
                        const notFoundPath = ((_definedFilePaths_find2 = definedFilePaths.find(([type])=>type === 'not-found')) == null ? void 0 : _definedFilePaths_find2[1]) ?? defaultNotFoundPath;
                        const varName = `notFound${nestedCollectedDeclarations.length}`;
                        nestedCollectedDeclarations.push([
                            varName,
                            notFoundPath
                        ]);
                        subtreeCode = `{
              children: [${JSON.stringify(UNDERSCORE_NOT_FOUND_ROUTE.slice(1))}, {
                children: ['${PAGE_SEGMENT_KEY}', {}, {
                  page: [
                    ${varName},
                    ${JSON.stringify(notFoundPath)}
                  ]
                }]
              }, {}]
            }`;
                    }
                }
            }
            // If it's app-error route, set app-error as children page
            if (isAppErrorRoute) {
                const varName = `appError${nestedCollectedDeclarations.length}`;
                nestedCollectedDeclarations.push([
                    varName,
                    appErrorPath
                ]);
                subtreeCode = `{
          children: [${JSON.stringify(UNDERSCORE_GLOBAL_ERROR_ROUTE.slice(1))}, {
            children: ['${PAGE_SEGMENT_KEY}', {}, {
              page: [
                ${varName},
                ${JSON.stringify(appErrorPath)}
              ]
            }]
          }, {}]
        }`;
            }
            // For 404 route
            // if global-not-found is in definedFilePaths, remove root layout for /_not-found,
            // and change it to global-not-found route.
            // TODO: remove this once global-not-found is stable.
            if (isNotFoundRoute && isGlobalNotFoundEnabled) {
                var _definedFilePaths_find3;
                definedFilePaths = definedFilePaths.filter(([type])=>type !== 'layout');
                // Replace the layout to global-not-found
                definedFilePaths.push([
                    'layout',
                    ((_definedFilePaths_find3 = definedFilePaths.find(([type])=>type === GLOBAL_NOT_FOUND_FILE_TYPE)) == null ? void 0 : _definedFilePaths_find3[1]) ?? defaultGlobalNotFoundPath
                ]);
            }
            if (isAppErrorRoute) {
                definedFilePaths = definedFilePaths.filter(([type])=>type !== 'layout');
            }
            const modulesCode = `{
        ${definedFilePaths.map(([file, filePath])=>{
                const varName = `module${nestedCollectedDeclarations.length}`;
                nestedCollectedDeclarations.push([
                    varName,
                    filePath
                ]);
                return `'${file}': [${varName}, ${JSON.stringify(filePath)}],`;
            }).join('\n')}
        ${createMetadataExportsCode(metadata)}
      }`;
            if (!subtreeCode) {
                const { treeCode: pageSubtreeCode } = await createSubtreePropsFromSegmentPath(subSegmentPath, nestedCollectedDeclarations);
                subtreeCode = pageSubtreeCode;
            }
            props[normalizedParallelKey] = `[
        '${parallelSegmentKey}',
        ${subtreeCode},
        ${modulesCode}
      ]`;
        }
        const adjacentParallelSegments = await resolveAdjacentParallelSegments(segmentPath);
        for (const adjacentParallelSegment of adjacentParallelSegments){
            if (!props[normalizeParallelKey(adjacentParallelSegment)]) {
                const actualSegment = adjacentParallelSegment === 'children' ? '' : `/${adjacentParallelSegment}`;
                // Use the default path if it's found, otherwise if it's a children
                // slot, then use the fallback (which triggers a `notFound()`). If this
                // isn't a children slot, then throw an error, as it produces a silent
                // 404 if we'd used the fallback.
                const fullSegmentPath = `${appDirPrefix}${segmentPath}${actualSegment}`;
                let defaultPath = await resolver(`${fullSegmentPath}/default`);
                if (!defaultPath) {
                    if (adjacentParallelSegment === 'children') {
                        // When we host applications on Vercel, the status code affects the
                        // underlying behavior of the route, which when we are missing the
                        // children slot of an interception route, will yield a full 404
                        // response for the RSC request instead. For this reason, we expect
                        // that if a default file is missing when we're rendering an
                        // interception route, we instead always render null for the default
                        // slot to avoid the full 404 response.
                        if (isInterceptionRouteAppPath(page)) {
                            defaultPath = PARALLEL_ROUTE_DEFAULT_NULL_PATH;
                        } else {
                            defaultPath = PARALLEL_ROUTE_DEFAULT_PATH;
                        }
                    } else {
                        // Check if we're inside a catch-all route (i.e., the parallel route is a child
                        // of a catch-all segment). Only skip validation if the slot is UNDER a catch-all.
                        // For example:
                        //   /[...catchAll]/@slot - isInsideCatchAll = true (skip validation) ✓
                        //   /@slot/[...catchAll] - isInsideCatchAll = false (require default) ✓
                        // The catch-all provides fallback behavior, so default.js is not required.
                        const isInsideCatchAll = segments.some(isCatchAllSegment);
                        // Check if this is a leaf segment (no child routes).
                        // Leaf segments don't need default.js because there are no child routes
                        // that could cause the parallel slot to unmatch. For example:
                        //   /repo-overview/@slot/page with no child routes - isLeafSegment = true (skip validation) ✓
                        //   /repo-overview/@slot/page with /repo-overview/child/page - isLeafSegment = false (require default) ✓
                        // This also handles route groups correctly by filtering them out.
                        const isLeafSegment = !hasChildRoutesForSegment(segmentPath);
                        if (!isInsideCatchAll && !isLeafSegment) {
                            // Replace internal webpack alias with user-facing directory name
                            const userFacingPath = fullSegmentPath.replace(APP_DIR_ALIAS, 'app');
                            throw new MissingDefaultParallelRouteError(userFacingPath, adjacentParallelSegment);
                        }
                        defaultPath = PARALLEL_ROUTE_DEFAULT_PATH;
                    }
                }
                const varName = `default${nestedCollectedDeclarations.length}`;
                nestedCollectedDeclarations.push([
                    varName,
                    defaultPath
                ]);
                props[normalizeParallelKey(adjacentParallelSegment)] = `[
          '${DEFAULT_SEGMENT_KEY}',
          {},
          {
            defaultPage: [${varName}, ${JSON.stringify(defaultPath)}],
          }
        ]`;
            }
        }
        return {
            treeCode: `{
        ${Object.entries(props).map(([key, value])=>`${key}: ${value}`).join(',\n')}
      }`
        };
    }
    const { treeCode } = await createSubtreePropsFromSegmentPath([], collectedDeclarations);
    return {
        treeCode: `${treeCode}.children;`,
        rootLayout,
        globalError,
        globalNotFound
    };
}
function createAbsolutePath(appDir, pathToTurnAbsolute) {
    return pathToTurnAbsolute// Replace all POSIX path separators with the current OS path separator
    .replace(/\//g, path.sep).replace(/^private-next-app-dir/, appDir);
}
const filesInDirMapMap = new WeakMap();
const nextAppLoader = async function nextAppLoader() {
    // install native bindings early so they are always available.
    // When run by webpack, next will have already done this, so this will be fast,
    // but if run by turbopack in a subprocess it is required.  In that case we cannot pass the
    // `useWasmBinary` flag, but that is ok since turbopack doesn't currently support wasm.
    await installBindings();
    const loaderOptions = this.getOptions();
    const { name, appDir, appPaths, pagePath, pageExtensions, rootDir, tsconfigPath, isDev, nextConfigOutput, preferredRegion, basePath, middlewareConfig: middlewareConfigBase64 } = loaderOptions;
    const isGlobalNotFoundEnabled = !!loaderOptions.isGlobalNotFoundEnabled;
    // Update FILE_TYPES on the very top-level of the loader
    if (!isGlobalNotFoundEnabled) {
        // @ts-expect-error this delete is only necessary while experimental
        delete FILE_TYPES['global-not-found'];
    }
    const buildInfo = getModuleBuildInfo(this._module);
    const collectedDeclarations = [];
    // Use the page from loaderOptions directly instead of deriving it from name.
    // The name (bundlePath) may have been normalized with normalizePagePath()
    // which is designed for Pages Router and incorrectly duplicates /index paths
    // (e.g., /index/page -> /index/index/page). The page parameter contains the
    // correct unnormalized value.
    const page = loaderOptions.page;
    const middlewareConfig = JSON.parse(Buffer.from(middlewareConfigBase64, 'base64').toString());
    buildInfo.route = {
        page,
        absolutePagePath: createAbsolutePath(appDir, pagePath),
        preferredRegion,
        middlewareConfig,
        relatedModules: []
    };
    const extensions = typeof pageExtensions === 'string' ? [
        pageExtensions
    ] : pageExtensions.map((extension)=>`.${extension}`);
    const normalizedAppPaths = typeof appPaths === 'string' ? [
        appPaths
    ] : appPaths || [];
    const resolveParallelSegments = (pathname)=>{
        const matched = {};
        let existingChildrenPath;
        for (const appPath of normalizedAppPaths){
            if (appPath.startsWith(pathname + '/')) {
                const rest = appPath.slice(pathname.length + 1).split('/');
                // It is the actual page, mark it specially.
                if (rest.length === 1 && rest[0] === 'page') {
                    existingChildrenPath = appPath;
                    matched.children = PAGE_SEGMENT;
                    continue;
                }
                const isParallelRoute = rest[0].startsWith('@');
                if (isParallelRoute) {
                    if (rest.length === 2 && rest[1] === 'page') {
                        // We found a parallel route at this level. We don't want to mark it explicitly as the page segment,
                        // as that should be matched to the `children` slot. Instead, we use an array, to signal to `createSubtreePropsFromSegmentPath`
                        // that it needs to recursively fill in the loader tree code for the parallel route at the appropriate levels.
                        matched[rest[0]] = [
                            PAGE_SEGMENT
                        ];
                        continue;
                    }
                    // If it was a parallel route but we weren't able to find the page segment (ie, maybe the page is nested further)
                    // we first insert a special marker to ensure that we still process layout/default/etc at the slot level prior to continuing
                    // on to the page segment.
                    matched[rest[0]] = [
                        PARALLEL_VIRTUAL_SEGMENT,
                        ...rest.slice(1)
                    ];
                    continue;
                }
                if (existingChildrenPath && matched.children !== rest[0]) {
                    // If we get here, it means we already set a `page` segment earlier in the loop,
                    // meaning we already matched a page to the `children` parallel segment.
                    const isIncomingParallelPage = appPath.includes('@');
                    const hasCurrentParallelPage = existingChildrenPath.includes('@');
                    if (isIncomingParallelPage) {
                        continue;
                    } else if (!hasCurrentParallelPage && !isIncomingParallelPage) {
                        // Both the current `children` and the incoming `children` are regular pages.
                        throw Object.defineProperty(new Error(`You cannot have two parallel pages that resolve to the same path. Please check ${existingChildrenPath} and ${appPath}. Refer to the route group docs for more information: https://nextjs.org/docs/app/building-your-application/routing/route-groups`), "__NEXT_ERROR_CODE", {
                            value: "E28",
                            enumerable: false,
                            configurable: true
                        });
                    }
                }
                existingChildrenPath = appPath;
                matched.children = rest[0];
            }
        }
        return Object.entries(matched);
    };
    const hasChildRoutesForSegment = (segmentPath)=>{
        const pathPrefix = segmentPath ? `${segmentPath}/` : '';
        for (const appPath of normalizedAppPaths){
            if (appPath.startsWith(pathPrefix)) {
                var _routeSegments_;
                const rest = appPath.slice(pathPrefix.length).split('/');
                // Filter out route groups to get the actual route segments
                // Route groups (e.g., "(group)") don't contribute to the URL path
                const routeSegments = rest.filter((segment)=>!isGroupSegment(segment));
                // If it's just 'page' at this level, skip (not a child route)
                if (routeSegments.length === 1 && routeSegments[0] === 'page') {
                    continue;
                }
                // If the first segment (after filtering route groups) is a parallel route, skip
                if ((_routeSegments_ = routeSegments[0]) == null ? void 0 : _routeSegments_.startsWith('@')) {
                    continue;
                }
                // If we have more than just 'page', then there are child routes
                // Examples:
                //   ['child', 'page'] -> true (has child route)
                //   ['page'] -> false (already filtered above)
                //   ['grandchild', 'deeper', 'page'] -> true (has nested child routes)
                if (routeSegments.length > 1 || routeSegments.length === 1 && routeSegments[0] !== 'page') {
                    return true;
                }
            }
        }
        return false;
    };
    const resolveDir = (pathToResolve)=>{
        return createAbsolutePath(appDir, pathToResolve);
    };
    const resolveAppRoute = (pathToResolve)=>{
        return createAbsolutePath(appDir, pathToResolve);
    };
    // Cached checker to see if a file exists in a given directory.
    // This can be more efficient than checking them with `fs.stat` one by one
    // because all the thousands of files are likely in a few possible directories.
    // Note that it should only be cached for this compilation, not globally.
    const fileExistsInDirectory = async (dirname, fileName)=>{
        // I don't think we should ever hit this code path, but if we do we should handle it gracefully.
        if (this._compilation === undefined) {
            try {
                return (await getFilesInDir(dirname).catch(()=>new Set())).has(fileName);
            } catch (e) {
                return false;
            }
        }
        const map = filesInDirMapMap.get(this._compilation) || new Map();
        if (!filesInDirMapMap.has(this._compilation)) {
            filesInDirMapMap.set(this._compilation, map);
        }
        if (!map.has(dirname)) {
            map.set(dirname, getFilesInDir(dirname).catch(()=>new Set()));
        }
        return (await map.get(dirname) || new Set()).has(fileName);
    };
    const resolver = async (pathname)=>{
        const absolutePath = createAbsolutePath(appDir, pathname);
        const filenameIndex = absolutePath.lastIndexOf(path.sep);
        const dirname = absolutePath.slice(0, filenameIndex);
        const filename = absolutePath.slice(filenameIndex + 1);
        let result;
        for (const ext of extensions){
            const absolutePathWithExtension = `${absolutePath}${ext}`;
            if (!result && await fileExistsInDirectory(dirname, `${filename}${ext}`)) {
                result = absolutePathWithExtension;
            }
            // Call `addMissingDependency` for all files even if they didn't match,
            // because they might be added or removed during development.
            this.addMissingDependency(absolutePathWithExtension);
        }
        return result;
    };
    const metadataResolver = async (dirname, filename, exts)=>{
        const absoluteDir = createAbsolutePath(appDir, dirname);
        let result;
        for (const ext of exts){
            // Compared to `resolver` above the exts do not have the `.` included already, so it's added here.
            const filenameWithExt = `${filename}.${ext}`;
            const absolutePathWithExtension = `${absoluteDir}${path.sep}${filenameWithExt}`;
            if (!result && await fileExistsInDirectory(dirname, filenameWithExt)) {
                result = absolutePathWithExtension;
            }
            // Call `addMissingDependency` for all files even if they didn't match,
            // because they might be added or removed during development.
            this.addMissingDependency(absolutePathWithExtension);
        }
        return result;
    };
    if (isAppRouteRoute(name)) {
        return createAppRouteCode({
            appDir,
            // TODO: investigate if the local `page` is the same as the loaderOptions.page
            page: loaderOptions.page,
            name,
            pagePath,
            resolveAppRoute,
            pageExtensions,
            nextConfigOutput
        });
    }
    let treeCodeResult = await createTreeCodeFromPath(pagePath, {
        page,
        resolveDir,
        resolver,
        metadataResolver,
        resolveParallelSegments,
        hasChildRoutesForSegment,
        loaderContext: this,
        pageExtensions,
        basePath,
        collectedDeclarations,
        isGlobalNotFoundEnabled
    });
    const isGlobalNotFoundPath = page === UNDERSCORE_NOT_FOUND_ROUTE_ENTRY && !!treeCodeResult.globalNotFound && isGlobalNotFoundEnabled;
    const isAppErrorRoute = page === UNDERSCORE_GLOBAL_ERROR_ROUTE_ENTRY;
    if (!treeCodeResult.rootLayout && !isGlobalNotFoundPath && !isAppErrorRoute) {
        if (!isDev) {
            // If we're building and missing a root layout, exit the build
            Log.error(`${bold(pagePath.replace(`${APP_DIR_ALIAS}/`, ''))} doesn't have a root layout. To fix this error, make sure every page has a root layout.`);
            process.exit(1);
        } else {
            var _filesInDirMapMap_get;
            // In dev we'll try to create a root layout
            const [createdRootLayout, rootLayoutPath] = await verifyRootLayout({
                appDir: appDir,
                dir: rootDir,
                tsconfigPath: tsconfigPath,
                pagePath,
                pageExtensions
            });
            if (!createdRootLayout) {
                let message = `${bold(pagePath.replace(`${APP_DIR_ALIAS}/`, ''))} doesn't have a root layout. `;
                if (rootLayoutPath) {
                    var _this__compiler;
                    message += `We tried to create ${bold(path.relative(((_this__compiler = this._compiler) == null ? void 0 : _this__compiler.context) ?? '', rootLayoutPath))} for you but something went wrong.`;
                } else {
                    message += 'To fix this error, make sure every page has a root layout.';
                }
                throw Object.defineProperty(new Error(message), "__NEXT_ERROR_CODE", {
                    value: "E394",
                    enumerable: false,
                    configurable: true
                });
            }
            // Clear fs cache, get the new result with the created root layout.
            if (this._compilation) (_filesInDirMapMap_get = filesInDirMapMap.get(this._compilation)) == null ? void 0 : _filesInDirMapMap_get.clear();
            treeCodeResult = await createTreeCodeFromPath(pagePath, {
                page,
                resolveDir,
                resolver,
                metadataResolver,
                resolveParallelSegments,
                hasChildRoutesForSegment,
                loaderContext: this,
                pageExtensions,
                basePath,
                collectedDeclarations,
                isGlobalNotFoundEnabled
            });
        }
    }
    const pathname = new AppPathnameNormalizer().normalize(page);
    // Prefer to modify next/src/server/app-render/entry-base.ts since this is shared with Turbopack.
    // Any changes to this code should be reflected in Turbopack's app_source.rs and/or app-renderer.tsx as well.
    const code = await loadEntrypoint('app-page', {
        VAR_DEFINITION_PAGE: page,
        VAR_DEFINITION_PATHNAME: pathname,
        VAR_MODULE_GLOBAL_ERROR: treeCodeResult.globalError
    }, {
        tree: treeCodeResult.treeCode,
        __next_app_require__: '__webpack_require__',
        // all modules are in the entry chunk, so we never actually need to load chunks in webpack
        __next_app_load_chunk__: '() => Promise.resolve()'
    });
    // Lazily evaluate the imported modules in the generated code
    const header = collectedDeclarations.map(([varName, modulePath])=>{
        return `const ${varName} = () => import(/* webpackMode: "eager" */ ${JSON.stringify(modulePath)});\n`;
    }).join('');
    return header + code;
};
export default nextAppLoader;

//# sourceMappingURL=index.js.map