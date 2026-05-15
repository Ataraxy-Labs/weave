const CachedParams = new WeakMap();
function makeUntrackedParams(underlyingParams) {
    const cachedParams = CachedParams.get(underlyingParams);
    if (cachedParams) {
        return cachedParams;
    }
    const promise = Promise.resolve(underlyingParams);
    CachedParams.set(underlyingParams, promise);
    return promise;
}
export function createRenderParamsFromClient(clientParams) {
    return makeUntrackedParams(clientParams);
}

//# sourceMappingURL=params.browser.prod.js.map