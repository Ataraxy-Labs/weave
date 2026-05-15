// Override chunk URL mapping in the webpack runtime
// https://github.com/webpack/webpack/blob/2738eebc7880835d88c727d364ad37f3ec557593/lib/RuntimeGlobals.js#L204
import { getDeploymentIdQueryOrEmptyString } from '../shared/lib/deployment-id';
import { encodeURIPath } from '../shared/lib/encode-uri-path';
// If we have a deployment ID, we need to append it to the webpack chunk names
// I am keeping the process check explicit so this can be statically optimized
if (process.env.NEXT_DEPLOYMENT_ID) {
    const suffix = getDeploymentIdQueryOrEmptyString();
    const getChunkScriptFilename = __webpack_require__.u;
    __webpack_require__.u = (...args)=>// We encode the chunk filename because our static server matches against and encoded
        // filename path.
        encodeURIPath(getChunkScriptFilename(...args)) + suffix;
    const getChunkCssFilename = __webpack_require__.k;
    __webpack_require__.k = (...args)=>getChunkCssFilename(...args) + suffix;
    const getMiniCssFilename = __webpack_require__.miniCssF;
    __webpack_require__.miniCssF = (...args)=>getMiniCssFilename(...args) + suffix;
} else {
    const getChunkScriptFilename = __webpack_require__.u;
    __webpack_require__.u = (...args)=>// We encode the chunk filename because our static server matches against and encoded
        // filename path.
        encodeURIPath(getChunkScriptFilename(...args));
// We don't need to override __webpack_require__.k because we don't modify
// the css chunk name when not using deployment id suffixes
// WE don't need to override __webpack_require__.miniCssF because we don't modify
// the mini css chunk name when not using deployment id suffixes
}

//# sourceMappingURL=app-webpack.js.map