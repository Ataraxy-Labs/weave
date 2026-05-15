export const createRenderSearchParamsFromClient = process.env.NODE_ENV === 'development' ? require('./search-params.browser.dev').createRenderSearchParamsFromClient : require('./search-params.browser.prod').createRenderSearchParamsFromClient;

//# sourceMappingURL=search-params.browser.js.map