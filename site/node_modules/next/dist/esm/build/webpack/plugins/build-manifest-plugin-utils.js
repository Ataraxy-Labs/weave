// Add the runtime ssg manifest file as a lazy-loaded file dependency.
// We also stub this file out for development mode (when it is not
// generated).
export const srcEmptySsgManifest = `self.__SSG_MANIFEST=new Set;self.__SSG_MANIFEST_CB&&self.__SSG_MANIFEST_CB()`;
function normalizeRewrite(item) {
    return {
        has: item.has,
        source: item.source,
        destination: item.destination
    };
}
export const processRoute = (r)=>{
    var _rewrite_destination;
    const rewrite = {
        ...r
    };
    // omit external rewrite destinations since these aren't
    // handled client-side
    if (!(rewrite == null ? void 0 : (_rewrite_destination = rewrite.destination) == null ? void 0 : _rewrite_destination.startsWith('/'))) {
        delete rewrite.destination;
    }
    return rewrite;
};
export function normalizeRewritesForBuildManifest(rewrites) {
    var _rewrites_afterFiles_map, _rewrites_afterFiles, _rewrites_beforeFiles_map, _rewrites_beforeFiles, _rewrites_fallback_map, _rewrites_fallback;
    return {
        afterFiles: (_rewrites_afterFiles = rewrites.afterFiles) == null ? void 0 : (_rewrites_afterFiles_map = _rewrites_afterFiles.map(processRoute)) == null ? void 0 : _rewrites_afterFiles_map.map((item)=>normalizeRewrite(item)),
        beforeFiles: (_rewrites_beforeFiles = rewrites.beforeFiles) == null ? void 0 : (_rewrites_beforeFiles_map = _rewrites_beforeFiles.map(processRoute)) == null ? void 0 : _rewrites_beforeFiles_map.map((item)=>normalizeRewrite(item)),
        fallback: (_rewrites_fallback = rewrites.fallback) == null ? void 0 : (_rewrites_fallback_map = _rewrites_fallback.map(processRoute)) == null ? void 0 : _rewrites_fallback_map.map((item)=>normalizeRewrite(item))
    };
}
export function createEdgeRuntimeManifest(originAssetMap) {
    const manifestFilenames = [
        '_buildManifest.js',
        '_ssgManifest.js'
    ];
    const assetMap = {
        ...originAssetMap,
        lowPriorityFiles: []
    };
    // we use globalThis here because middleware can be node
    // which doesn't have "self"
    const manifestDefCode = `globalThis.__BUILD_MANIFEST = ${JSON.stringify(assetMap, null, 2)};\n`;
    // edge lowPriorityFiles item: '"/static/" + process.env.__NEXT_BUILD_ID + "/low-priority.js"'.
    // Since lowPriorityFiles is not fixed and relying on `process.env.__NEXT_BUILD_ID`, we'll produce code creating it dynamically.
    const lowPriorityFilesCode = `globalThis.__BUILD_MANIFEST.lowPriorityFiles = [\n` + manifestFilenames.map((filename)=>{
        return `"/static/" + process.env.__NEXT_BUILD_ID + "/${filename}"`;
    }).join(',\n') + `\n];`;
    return manifestDefCode + lowPriorityFilesCode;
}

//# sourceMappingURL=build-manifest-plugin-utils.js.map