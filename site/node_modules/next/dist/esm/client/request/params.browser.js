export const createRenderParamsFromClient = process.env.NODE_ENV === 'development' ? require('./params.browser.dev').createRenderParamsFromClient : require('./params.browser.prod').createRenderParamsFromClient;

//# sourceMappingURL=params.browser.js.map