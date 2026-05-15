import { findSourceMap } from 'module';
import path from 'path';
import { fileURLToPath, pathToFileURL } from 'url';
import { SourceMapConsumer } from 'next/dist/compiled/source-map08';
import { getSourceMapFromFile } from './get-source-map-from-file';
import { devirtualizeReactServerURL, findApplicableSourceMapPayload, sourceMapIgnoreListsEverything } from '../lib/source-maps';
import { openFileInEditor } from '../../next-devtools/server/launch-editor';
import { getOriginalCodeFrame, ignoreListAnonymousStackFramesIfSandwiched } from '../../next-devtools/server/shared';
import { middlewareResponse } from '../../next-devtools/server/middleware-response';
import { formatFrameSourceFile } from '../../next-devtools/shared/webpack-module-path';
import { inspect } from 'util';
function shouldIgnoreSource(sourceURL) {
    return sourceURL.includes('node_modules') || // Only relevant for when Next.js is symlinked e.g. in the Next.js monorepo
    sourceURL.includes('next/dist') || sourceURL.startsWith('node:');
}
function getModuleById(id, compilation) {
    const { chunkGraph, modules } = compilation;
    return [
        ...modules
    ].find((module)=>chunkGraph.getModuleId(module) === id);
}
function findModuleNotFoundFromError(errorMessage) {
    var _errorMessage_match;
    return errorMessage == null ? void 0 : (_errorMessage_match = errorMessage.match(/'([^']+)' module/)) == null ? void 0 : _errorMessage_match[1];
}
function getSourcePath(source) {
    if (source.startsWith('file://')) {
        return fileURLToPath(source);
    }
    return source.replace(/^(webpack:\/\/\/|webpack:\/\/|webpack:\/\/_N_E\/)/, '');
}
/**
 * @returns 1-based lines and 0-based columns
 */ async function findOriginalSourcePositionAndContent(sourceMap, position) {
    let consumer;
    try {
        consumer = await new SourceMapConsumer(sourceMap);
    } catch (cause) {
        console.error(Object.defineProperty(new Error(`${sourceMap.file}: Invalid source map. Only conformant source maps can be used to find the original code.`, {
            cause
        }), "__NEXT_ERROR_CODE", {
            value: "E635",
            enumerable: false,
            configurable: true
        }));
        return null;
    }
    try {
        const sourcePosition = consumer.originalPositionFor({
            line: position.line1 ?? 1,
            // 0-based columns out requires 0-based columns in.
            column: (position.column1 ?? 1) - 1
        });
        if (!sourcePosition.source) {
            return null;
        }
        const sourceContent = consumer.sourceContentFor(sourcePosition.source, /* returnNullOnMissing */ true) ?? null;
        return {
            sourcePosition,
            sourceContent
        };
    } finally{
        consumer.destroy();
    }
}
export function getIgnoredSources(sourceMap) {
    const ignoreList = new Set(sourceMap.ignoreList ?? []);
    const moduleFilenames = (sourceMap == null ? void 0 : sourceMap.sources) ?? [];
    for(let index = 0; index < moduleFilenames.length; index++){
        // bundlerFilePath case: webpack://./app/page.tsx
        const webpackSourceURL = moduleFilenames[index];
        // Format the path to the normal file path
        const formattedFilePath = formatFrameSourceFile(webpackSourceURL);
        if (shouldIgnoreSource(formattedFilePath)) {
            ignoreList.add(index);
        }
    }
    const ignoredSources = sourceMap.sources.map((source, index)=>{
        var _sourceMap_sourcesContent;
        return {
            url: source,
            ignored: ignoreList.has(sourceMap.sources.indexOf(source)),
            content: ((_sourceMap_sourcesContent = sourceMap.sourcesContent) == null ? void 0 : _sourceMap_sourcesContent[index]) ?? null
        };
    });
    return ignoredSources;
}
function isIgnoredSource(source, sourcePosition) {
    if (sourcePosition.source == null) {
        return true;
    }
    for (const ignoredSource of source.ignoredSources){
        if (ignoredSource.ignored && ignoredSource.url === sourcePosition.source) {
            return true;
        }
    }
    return false;
}
function findOriginalSourcePositionAndContentFromCompilation(moduleId, importedModule, compilation) {
    var _module_buildInfo_importLocByPath, _module_buildInfo;
    const module = getModuleById(moduleId, compilation);
    return (module == null ? void 0 : (_module_buildInfo = module.buildInfo) == null ? void 0 : (_module_buildInfo_importLocByPath = _module_buildInfo.importLocByPath) == null ? void 0 : _module_buildInfo_importLocByPath.get(importedModule)) ?? null;
}
export async function createOriginalStackFrame({ ignoredByDefault, source, rootDirectory, frame, errorMessage }) {
    var // We ignore the sourcemapped name since it won't be the correct name.
    // The callsite will point to the column of the variable name instead of the
    // name of the enclosing function.
    // TODO(NDX-531): Spy on prepareStackTrace to get the enclosing line number for method name mapping.
    // default is not a valid identifier in JS so webpack uses a custom variable when it's an unnamed default export
    // Resolve it back to `default` for the method name if the source position didn't have the method.
    _frame_methodName_replace, _frame_methodName;
    const moduleNotFound = findModuleNotFoundFromError(errorMessage);
    const result = await (()=>{
        if (moduleNotFound) {
            if (source.type === 'file') {
                return undefined;
            }
            return findOriginalSourcePositionAndContentFromCompilation(source.moduleId, moduleNotFound, source.compilation);
        }
        return findOriginalSourcePositionAndContent(source.sourceMap, frame);
    })();
    if (!result) {
        return null;
    }
    const { sourcePosition, sourceContent } = result;
    if (!sourcePosition.source) {
        return null;
    }
    const ignored = ignoredByDefault || isIgnoredSource(source, sourcePosition) || // If the source file is externals, should be excluded even it's not ignored source.
    // e.g. webpack://next/dist/.. needs to be ignored
    shouldIgnoreSource(source.moduleURL);
    const sourcePath = getSourcePath(// When sourcePosition.source is the loader path the modulePath is generally better.
    (sourcePosition.source.includes('|') ? source.moduleURL : sourcePosition.source) || source.moduleURL);
    const filePath = path.resolve(rootDirectory, sourcePath);
    const resolvedFilePath = path.relative(rootDirectory, filePath);
    const traced = {
        file: resolvedFilePath,
        line1: sourcePosition.line,
        column1: sourcePosition.column === null ? null : sourcePosition.column + 1,
        methodName: (_frame_methodName = frame.methodName) == null ? void 0 : (_frame_methodName_replace = _frame_methodName.replace('__WEBPACK_DEFAULT_EXPORT__', 'default')) == null ? void 0 : _frame_methodName_replace.replace('__webpack_exports__.', ''),
        arguments: [],
        ignored
    };
    return {
        originalStackFrame: traced,
        originalCodeFrame: getOriginalCodeFrame(traced, sourceContent)
    };
}
async function getSourceMapFromCompilation(id, compilation) {
    try {
        const module = getModuleById(id, compilation);
        if (!module) {
            return undefined;
        }
        // @ts-expect-error The types for `CodeGenerationResults.get` require a
        // runtime to be passed as second argument, but apparently it also works
        // without it.
        const codeGenerationResult = compilation.codeGenerationResults.get(module);
        const source = codeGenerationResult == null ? void 0 : codeGenerationResult.sources.get('javascript');
        return (source == null ? void 0 : source.map()) ?? undefined;
    } catch (err) {
        console.error(`Failed to lookup module by ID ("${id}"):`, err);
        return undefined;
    }
}
async function getSource(frame, options) {
    let sourceURL = frame.file ?? '';
    const { getCompilations } = options;
    sourceURL = devirtualizeReactServerURL(sourceURL);
    let nativeSourceMap;
    try {
        nativeSourceMap = findSourceMap(sourceURL);
    } catch (cause) {
        throw Object.defineProperty(new Error(`${sourceURL}: Invalid source map. Only conformant source maps can be used to find the original code.`, {
            cause
        }), "__NEXT_ERROR_CODE", {
            value: "E635",
            enumerable: false,
            configurable: true
        });
    }
    if (nativeSourceMap !== undefined) {
        const sourceMapPayload = nativeSourceMap.payload;
        return {
            type: 'file',
            sourceMap: findApplicableSourceMapPayload((frame.line1 ?? 1) - 1, (frame.column1 ?? 1) - 1, sourceMapPayload),
            ignoredSources: getIgnoredSources(// @ts-expect-error -- TODO: Support IndexSourceMap
            sourceMapPayload),
            moduleURL: sourceURL
        };
    }
    if (path.isAbsolute(sourceURL)) {
        sourceURL = pathToFileURL(sourceURL).href;
    }
    if (sourceURL.startsWith('file:')) {
        const sourceMap = await getSourceMapFromFile(sourceURL);
        return sourceMap ? {
            type: 'file',
            sourceMap,
            ignoredSources: getIgnoredSources(sourceMap),
            moduleURL: sourceURL
        } : undefined;
    }
    // webpack-internal:///./src/hello.tsx => ./src/hello.tsx
    // webpack://_N_E/./src/hello.tsx => ./src/hello.tsx
    const moduleId = sourceURL.replace(/^(webpack-internal:\/\/\/|webpack:\/\/(_N_E\/)?)/, '').replace(/\?\d+$/, '');
    // (rsc)/./src/hello.tsx => ./src/hello.tsx
    const moduleURL = moduleId.replace(/^(\(.*\)\/?)/, '');
    for (const compilation of getCompilations()){
        const sourceMap = await getSourceMapFromCompilation(moduleId, compilation);
        if (sourceMap) {
            const ignoredSources = getIgnoredSources(sourceMap);
            return {
                type: 'bundle',
                sourceMap,
                compilation,
                moduleId,
                moduleURL,
                ignoredSources
            };
        }
    }
    return undefined;
}
export async function getOriginalStackFrames({ isServer, isEdgeServer, isAppDirectory, frames, clientStats, serverStats, edgeServerStats, rootDirectory }) {
    const frameResponses = await Promise.all(frames.map((frame)=>getOriginalStackFrame({
            isServer,
            isEdgeServer,
            isAppDirectory,
            frame,
            clientStats,
            serverStats,
            edgeServerStats,
            rootDirectory
        }).then((value)=>{
            return {
                status: 'fulfilled',
                value
            };
        }, (reason)=>{
            return {
                status: 'rejected',
                reason: inspect(reason, {
                    colors: false
                })
            };
        })));
    ignoreListAnonymousStackFramesIfSandwiched(frameResponses);
    return frameResponses;
}
async function getOriginalStackFrame({ isServer, isEdgeServer, isAppDirectory, frame, clientStats, serverStats, edgeServerStats, rootDirectory }) {
    const filename = frame.file ?? '';
    const source = await getSource(frame, {
        getCompilations: ()=>{
            const compilations = [];
            // Try Client Compilation first. In `pages` we leverage
            // `isClientError` to check. In `app` it depends on if it's a server
            // / client component and when the code throws. E.g. during HTML
            // rendering it's the server/edge compilation.
            if (!isEdgeServer && !isServer || isAppDirectory) {
                var _clientStats;
                const compilation = (_clientStats = clientStats()) == null ? void 0 : _clientStats.compilation;
                if (compilation) {
                    compilations.push(compilation);
                }
            }
            // Try Server Compilation. In `pages` this could be something
            // imported in getServerSideProps/getStaticProps as the code for
            // those is tree-shaken. In `app` this finds server components and
            // code that was imported from a server component. It also covers
            // when client component code throws during HTML rendering.
            if (isServer || isAppDirectory) {
                var _serverStats;
                const compilation = (_serverStats = serverStats()) == null ? void 0 : _serverStats.compilation;
                if (compilation) {
                    compilations.push(compilation);
                }
            }
            // Try Edge Server Compilation. Both cases are the same as Server
            // Compilation, main difference is that it covers `runtime: 'edge'`
            // pages/app routes.
            if (isEdgeServer || isAppDirectory) {
                var _edgeServerStats;
                const compilation = (_edgeServerStats = edgeServerStats()) == null ? void 0 : _edgeServerStats.compilation;
                if (compilation) {
                    compilations.push(compilation);
                }
            }
            return compilations;
        }
    });
    let defaultNormalizedStackFrameLocation = frame.file;
    if (defaultNormalizedStackFrameLocation !== null && defaultNormalizedStackFrameLocation.startsWith('file://')) {
        defaultNormalizedStackFrameLocation = path.relative(rootDirectory, fileURLToPath(defaultNormalizedStackFrameLocation));
    }
    // This stack frame is used for the one that couldn't locate the source or source mapped frame
    const defaultStackFrame = {
        file: defaultNormalizedStackFrameLocation,
        line1: frame.line1,
        column1: frame.column1,
        methodName: frame.methodName,
        ignored: shouldIgnoreSource(filename),
        arguments: []
    };
    if (!source) {
        // return original stack frame with no source map
        return {
            originalStackFrame: defaultStackFrame,
            originalCodeFrame: null
        };
    }
    defaultStackFrame.ignored ||= sourceMapIgnoreListsEverything(source.sourceMap);
    const originalStackFrameResponse = await createOriginalStackFrame({
        ignoredByDefault: defaultStackFrame.ignored,
        frame,
        source,
        rootDirectory
    });
    if (!originalStackFrameResponse) {
        return {
            originalStackFrame: defaultStackFrame,
            originalCodeFrame: null
        };
    }
    return originalStackFrameResponse;
}
export function getOverlayMiddleware(options) {
    const { rootDirectory, isSrcDir, clientStats, serverStats, edgeServerStats } = options;
    return async function(req, res, next) {
        const { pathname, searchParams } = new URL(`http://n${req.url}`);
        if (pathname === '/__nextjs_original-stack-frames') {
            if (req.method !== 'POST') {
                return middlewareResponse.badRequest(res);
            }
            const body = await new Promise((resolve, reject)=>{
                let data = '';
                req.on('data', (chunk)=>{
                    data += chunk;
                });
                req.on('end', ()=>resolve(data));
                req.on('error', reject);
            });
            try {
                const { frames, isServer, isEdgeServer, isAppDirectory } = JSON.parse(body);
                return middlewareResponse.json(res, await getOriginalStackFrames({
                    isServer,
                    isEdgeServer,
                    isAppDirectory,
                    frames,
                    clientStats,
                    serverStats,
                    edgeServerStats,
                    rootDirectory
                }));
            } catch (err) {
                return middlewareResponse.badRequest(res);
            }
        } else if (pathname === '/__nextjs_launch-editor') {
            const frame = {
                file: searchParams.get('file'),
                methodName: searchParams.get('methodName'),
                line1: parseInt(searchParams.get('line1') ?? '1', 10) || 1,
                column1: parseInt(searchParams.get('column1') ?? '1', 10) || 1,
                arguments: searchParams.getAll('arguments').filter(Boolean)
            };
            if (!frame.file) return middlewareResponse.badRequest(res);
            let openEditorResult;
            const isAppRelativePath = searchParams.get('isAppRelativePath') === '1';
            if (isAppRelativePath) {
                const relativeFilePath = searchParams.get('file') || '';
                const appPath = path.join('app', isSrcDir ? 'src' : '', relativeFilePath);
                openEditorResult = await openFileInEditor(appPath, 1, 1, rootDirectory);
            } else {
                // TODO: How do we differentiate layers and actual file paths with round brackets?
                // frame files may start with their webpack layer, like (middleware)/middleware.js
                const filePath = frame.file.replace(/^\([^)]+\)\//, '');
                openEditorResult = await openFileInEditor(filePath, frame.line1, frame.column1 ?? 1, rootDirectory);
            }
            if (openEditorResult.error) {
                console.error('Failed to launch editor:', openEditorResult.error);
                return middlewareResponse.internalServerError(res, openEditorResult.error);
            }
            if (!openEditorResult.found) {
                return middlewareResponse.notFound(res);
            }
            return middlewareResponse.noContent(res);
        }
        return next();
    };
}
export function getSourceMapMiddleware(options) {
    const { clientStats, serverStats, edgeServerStats } = options;
    return async function(req, res, next) {
        const { pathname, searchParams } = new URL(`http://n${req.url}`);
        if (pathname !== '/__nextjs_source-map') {
            return next();
        }
        const filename = searchParams.get('filename');
        if (!filename) {
            return middlewareResponse.badRequest(res);
        }
        let source;
        try {
            source = await getSource({
                file: filename,
                // Webpack doesn't use Index Source Maps
                line1: null,
                column1: null
            }, {
                getCompilations: ()=>{
                    const compilations = [];
                    for (const stats of [
                        clientStats(),
                        serverStats(),
                        edgeServerStats()
                    ]){
                        if (stats == null ? void 0 : stats.compilation) {
                            compilations.push(stats.compilation);
                        }
                    }
                    return compilations;
                }
            });
        } catch (error) {
            return middlewareResponse.internalServerError(res, error);
        }
        if (!source) {
            return middlewareResponse.noContent(res);
        }
        return middlewareResponse.json(res, source.sourceMap);
    };
}

//# sourceMappingURL=middleware-webpack.js.map