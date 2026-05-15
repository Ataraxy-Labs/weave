import { getDeploymentId, getDeploymentIdQueryOrEmptyString } from '../shared/lib/deployment-id';
// If we have a deployment ID, we need to append it to the webpack chunk names
// I am keeping the process check explicit so this can be statically optimized
if (getDeploymentId()) {
    const suffix = getDeploymentIdQueryOrEmptyString();
    const getChunkScriptFilename = __webpack_require__.u;
    __webpack_require__.u = (...args)=>// We enode the chunk filename because our static server matches against and encoded
        // filename path.
        getChunkScriptFilename(...args) + suffix;
    const getChunkCssFilename = __webpack_require__.k;
    __webpack_require__.k = (...args)=>getChunkCssFilename(...args) + suffix;
    const getMiniCssFilename = __webpack_require__.miniCssF;
    __webpack_require__.miniCssF = (...args)=>getMiniCssFilename(...args) + suffix;
}
self.__next_set_public_path__ = (path)=>{
    __webpack_public_path__ = path;
};

//# sourceMappingURL=webpack.js.map