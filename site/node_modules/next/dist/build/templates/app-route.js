"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
0 && (module.exports = {
    handler: null,
    patchFetch: null,
    routeModule: null,
    serverHooks: null,
    workAsyncStorage: null,
    workUnitAsyncStorage: null
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: all[name]
    });
}
_export(exports, {
    handler: function() {
        return handler;
    },
    patchFetch: function() {
        return patchFetch;
    },
    routeModule: function() {
        return routeModule;
    },
    serverHooks: function() {
        return serverHooks;
    },
    workAsyncStorage: function() {
        return workAsyncStorage;
    },
    workUnitAsyncStorage: function() {
        return workUnitAsyncStorage;
    }
});
const _modulecompiled = require("../../server/route-modules/app-route/module.compiled");
const _routekind = require("../../server/route-kind");
const _patchfetch = require("../../server/lib/patch-fetch");
const _requestmeta = require("../../server/request-meta");
const _tracer = require("../../server/lib/trace/tracer");
const _manifestssingleton = require("../../server/app-render/manifests-singleton");
const _apppaths = require("../../shared/lib/router/utils/app-paths");
const _node = require("../../server/base-http/node");
const _nextrequest = require("../../server/web/spec-extension/adapters/next-request");
const _constants = require("../../server/lib/trace/constants");
const _utils = require("../../server/instrumentation/utils");
const _sendresponse = require("../../server/send-response");
const _utils1 = require("../../server/web/utils");
const _cachecontrol = require("../../server/lib/cache-control");
const _constants1 = require("../../lib/constants");
const _nofallbackerrorexternal = require("../../shared/lib/no-fallback-error.external");
const _responsecache = require("../../server/response-cache");
const _VAR_USERLAND = /*#__PURE__*/ _interop_require_wildcard(require("VAR_USERLAND"));
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
// We inject the nextConfigOutput here so that we can use them in the route
// module.
// INJECT:nextConfigOutput
const routeModule = new _modulecompiled.AppRouteRouteModule({
    definition: {
        kind: _routekind.RouteKind.APP_ROUTE,
        page: 'VAR_DEFINITION_PAGE',
        pathname: 'VAR_DEFINITION_PATHNAME',
        filename: 'VAR_DEFINITION_FILENAME',
        bundlePath: 'VAR_DEFINITION_BUNDLE_PATH'
    },
    distDir: process.env.__NEXT_RELATIVE_DIST_DIR || '',
    relativeProjectDir: process.env.__NEXT_RELATIVE_PROJECT_DIR || '',
    resolvedPagePath: 'VAR_RESOLVED_PAGE_PATH',
    nextConfigOutput,
    userland: _VAR_USERLAND
});
// Pull out the exports that we need to expose from the module. This should
// be eliminated when we've moved the other routes to the new format. These
// are used to hook into the route.
const { workAsyncStorage, workUnitAsyncStorage, serverHooks } = routeModule;
function patchFetch() {
    return (0, _patchfetch.patchFetch)({
        workAsyncStorage,
        workUnitAsyncStorage
    });
}
async function handler(req, res, ctx) {
    if (routeModule.isDev) {
        (0, _requestmeta.addRequestMeta)(req, 'devRequestTimingInternalsEnd', process.hrtime.bigint());
    }
    let srcPage = 'VAR_DEFINITION_PAGE';
    // turbopack doesn't normalize `/index` in the page name
    // so we need to to process dynamic routes properly
    // TODO: fix turbopack providing differing value from webpack
    if (process.env.TURBOPACK) {
        srcPage = srcPage.replace(/\/index$/, '') || '/';
    } else if (srcPage === '/index') {
        // we always normalize /index specifically
        srcPage = '/';
    }
    const multiZoneDraftMode = process.env.__NEXT_MULTI_ZONE_DRAFT_MODE;
    const prepareResult = await routeModule.prepare(req, res, {
        srcPage,
        multiZoneDraftMode
    });
    if (!prepareResult) {
        res.statusCode = 400;
        res.end('Bad Request');
        ctx.waitUntil == null ? void 0 : ctx.waitUntil.call(ctx, Promise.resolve());
        return null;
    }
    const { buildId, params, nextConfig, parsedUrl, isDraftMode, prerenderManifest, routerServerContext, isOnDemandRevalidate, revalidateOnlyGenerated, resolvedPathname, clientReferenceManifest, serverActionsManifest } = prepareResult;
    const normalizedSrcPage = (0, _apppaths.normalizeAppPath)(srcPage);
    let isIsr = Boolean(prerenderManifest.dynamicRoutes[normalizedSrcPage] || prerenderManifest.routes[resolvedPathname]);
    const render404 = async ()=>{
        // TODO: should route-module itself handle rendering the 404
        if (routerServerContext == null ? void 0 : routerServerContext.render404) {
            await routerServerContext.render404(req, res, parsedUrl, false);
        } else {
            res.end('This page could not be found');
        }
        return null;
    };
    if (isIsr && !isDraftMode) {
        const isPrerendered = Boolean(prerenderManifest.routes[resolvedPathname]);
        const prerenderInfo = prerenderManifest.dynamicRoutes[normalizedSrcPage];
        if (prerenderInfo) {
            if (prerenderInfo.fallback === false && !isPrerendered) {
                if (nextConfig.experimental.adapterPath) {
                    return await render404();
                }
                throw new _nofallbackerrorexternal.NoFallbackError();
            }
        }
    }
    let cacheKey = null;
    if (isIsr && !routeModule.isDev && !isDraftMode) {
        cacheKey = resolvedPathname;
        // ensure /index and / is normalized to one key
        cacheKey = cacheKey === '/index' ? '/' : cacheKey;
    }
    const supportsDynamicResponse = // If we're in development, we always support dynamic HTML
    routeModule.isDev === true || // If this is not SSG or does not have static paths, then it supports
    // dynamic HTML.
    !isIsr;
    // This is a revalidation request if the request is for a static
    // page and it is not being resumed from a postponed render and
    // it is not a dynamic RSC request then it is a revalidation
    // request.
    const isStaticGeneration = isIsr && !supportsDynamicResponse;
    // Before rendering (which initializes component tree modules), we have to
    // set the reference manifests to our global store so Server Action's
    // encryption util can access to them at the top level of the page module.
    if (serverActionsManifest && clientReferenceManifest) {
        (0, _manifestssingleton.setManifestsSingleton)({
            page: srcPage,
            clientReferenceManifest,
            serverActionsManifest
        });
    }
    const method = req.method || 'GET';
    const tracer = (0, _tracer.getTracer)();
    const activeSpan = tracer.getActiveScopeSpan();
    const context = {
        params,
        prerenderManifest,
        renderOpts: {
            experimental: {
                authInterrupts: Boolean(nextConfig.experimental.authInterrupts)
            },
            cacheComponents: Boolean(nextConfig.cacheComponents),
            supportsDynamicResponse,
            incrementalCache: (0, _requestmeta.getRequestMeta)(req, 'incrementalCache'),
            cacheLifeProfiles: nextConfig.cacheLife,
            waitUntil: ctx.waitUntil,
            onClose: (cb)=>{
                res.on('close', cb);
            },
            onAfterTaskError: undefined,
            onInstrumentationRequestError: (error, _request, errorContext, silenceLog)=>routeModule.onRequestError(req, error, errorContext, silenceLog, routerServerContext)
        },
        sharedContext: {
            buildId
        }
    };
    const nodeNextReq = new _node.NodeNextRequest(req);
    const nodeNextRes = new _node.NodeNextResponse(res);
    const nextReq = _nextrequest.NextRequestAdapter.fromNodeNextRequest(nodeNextReq, (0, _nextrequest.signalFromNodeResponse)(res));
    try {
        const invokeRouteModule = async (span)=>{
            return routeModule.handle(nextReq, context).finally(()=>{
                if (!span) return;
                span.setAttributes({
                    'http.status_code': res.statusCode,
                    'next.rsc': false
                });
                const rootSpanAttributes = tracer.getRootSpanAttributes();
                // We were unable to get attributes, probably OTEL is not enabled
                if (!rootSpanAttributes) {
                    return;
                }
                if (rootSpanAttributes.get('next.span_type') !== _constants.BaseServerSpan.handleRequest) {
                    console.warn(`Unexpected root span type '${rootSpanAttributes.get('next.span_type')}'. Please report this Next.js issue https://github.com/vercel/next.js`);
                    return;
                }
                const route = rootSpanAttributes.get('next.route');
                if (route) {
                    const name = `${method} ${route}`;
                    span.setAttributes({
                        'next.route': route,
                        'http.route': route,
                        'next.span_name': name
                    });
                    span.updateName(name);
                } else {
                    span.updateName(`${method} ${srcPage}`);
                }
            });
        };
        const isMinimalMode = Boolean(process.env.MINIMAL_MODE || (0, _requestmeta.getRequestMeta)(req, 'minimalMode'));
        const handleResponse = async (currentSpan)=>{
            var _cacheEntry_value;
            const responseGenerator = async ({ previousCacheEntry })=>{
                try {
                    if (!isMinimalMode && isOnDemandRevalidate && revalidateOnlyGenerated && !previousCacheEntry) {
                        res.statusCode = 404;
                        // on-demand revalidate always sets this header
                        res.setHeader('x-nextjs-cache', 'REVALIDATED');
                        res.end('This page could not be found');
                        return null;
                    }
                    const response = await invokeRouteModule(currentSpan);
                    req.fetchMetrics = context.renderOpts.fetchMetrics;
                    let pendingWaitUntil = context.renderOpts.pendingWaitUntil;
                    // Attempt using provided waitUntil if available
                    // if it's not we fallback to sendResponse's handling
                    if (pendingWaitUntil) {
                        if (ctx.waitUntil) {
                            ctx.waitUntil(pendingWaitUntil);
                            pendingWaitUntil = undefined;
                        }
                    }
                    const cacheTags = context.renderOpts.collectedTags;
                    // If the request is for a static response, we can cache it so long
                    // as it's not edge.
                    if (isIsr) {
                        const blob = await response.blob();
                        // Copy the headers from the response.
                        const headers = (0, _utils1.toNodeOutgoingHttpHeaders)(response.headers);
                        if (cacheTags) {
                            headers[_constants1.NEXT_CACHE_TAGS_HEADER] = cacheTags;
                        }
                        if (!headers['content-type'] && blob.type) {
                            headers['content-type'] = blob.type;
                        }
                        const revalidate = typeof context.renderOpts.collectedRevalidate === 'undefined' || context.renderOpts.collectedRevalidate >= _constants1.INFINITE_CACHE ? false : context.renderOpts.collectedRevalidate;
                        const expire = typeof context.renderOpts.collectedExpire === 'undefined' || context.renderOpts.collectedExpire >= _constants1.INFINITE_CACHE ? undefined : context.renderOpts.collectedExpire;
                        // Create the cache entry for the response.
                        const cacheEntry = {
                            value: {
                                kind: _responsecache.CachedRouteKind.APP_ROUTE,
                                status: response.status,
                                body: Buffer.from(await blob.arrayBuffer()),
                                headers
                            },
                            cacheControl: {
                                revalidate,
                                expire
                            }
                        };
                        return cacheEntry;
                    } else {
                        // send response without caching if not ISR
                        await (0, _sendresponse.sendResponse)(nodeNextReq, nodeNextRes, response, context.renderOpts.pendingWaitUntil);
                        return null;
                    }
                } catch (err) {
                    // if this is a background revalidate we need to report
                    // the request error here as it won't be bubbled
                    if (previousCacheEntry == null ? void 0 : previousCacheEntry.isStale) {
                        const silenceLog = false;
                        await routeModule.onRequestError(req, err, {
                            routerKind: 'App Router',
                            routePath: srcPage,
                            routeType: 'route',
                            revalidateReason: (0, _utils.getRevalidateReason)({
                                isStaticGeneration,
                                isOnDemandRevalidate
                            })
                        }, silenceLog, routerServerContext);
                    }
                    throw err;
                }
            };
            const cacheEntry = await routeModule.handleResponse({
                req,
                nextConfig,
                cacheKey,
                routeKind: _routekind.RouteKind.APP_ROUTE,
                isFallback: false,
                prerenderManifest,
                isRoutePPREnabled: false,
                isOnDemandRevalidate,
                revalidateOnlyGenerated,
                responseGenerator,
                waitUntil: ctx.waitUntil,
                isMinimalMode
            });
            // we don't create a cacheEntry for ISR
            if (!isIsr) {
                return null;
            }
            if ((cacheEntry == null ? void 0 : (_cacheEntry_value = cacheEntry.value) == null ? void 0 : _cacheEntry_value.kind) !== _responsecache.CachedRouteKind.APP_ROUTE) {
                var _cacheEntry_value1;
                throw Object.defineProperty(new Error(`Invariant: app-route received invalid cache entry ${cacheEntry == null ? void 0 : (_cacheEntry_value1 = cacheEntry.value) == null ? void 0 : _cacheEntry_value1.kind}`), "__NEXT_ERROR_CODE", {
                    value: "E701",
                    enumerable: false,
                    configurable: true
                });
            }
            if (!isMinimalMode) {
                res.setHeader('x-nextjs-cache', isOnDemandRevalidate ? 'REVALIDATED' : cacheEntry.isMiss ? 'MISS' : cacheEntry.isStale ? 'STALE' : 'HIT');
            }
            // Draft mode should never be cached
            if (isDraftMode) {
                res.setHeader('Cache-Control', 'private, no-cache, no-store, max-age=0, must-revalidate');
            }
            const headers = (0, _utils1.fromNodeOutgoingHttpHeaders)(cacheEntry.value.headers);
            if (!(isMinimalMode && isIsr)) {
                headers.delete(_constants1.NEXT_CACHE_TAGS_HEADER);
            }
            // If cache control is already set on the response we don't
            // override it to allow users to customize it via next.config
            if (cacheEntry.cacheControl && !res.getHeader('Cache-Control') && !headers.get('Cache-Control')) {
                headers.set('Cache-Control', (0, _cachecontrol.getCacheControlHeader)(cacheEntry.cacheControl));
            }
            await (0, _sendresponse.sendResponse)(nodeNextReq, nodeNextRes, // @ts-expect-error - Argument of type 'Buffer<ArrayBufferLike>' is not assignable to parameter of type 'BodyInit | null | undefined'.
            new Response(cacheEntry.value.body, {
                headers,
                status: cacheEntry.value.status || 200
            }));
            return null;
        };
        // TODO: activeSpan code path is for when wrapped by
        // next-server can be removed when this is no longer used
        if (activeSpan) {
            await handleResponse(activeSpan);
        } else {
            await tracer.withPropagatedContext(req.headers, ()=>tracer.trace(_constants.BaseServerSpan.handleRequest, {
                    spanName: `${method} ${srcPage}`,
                    kind: _tracer.SpanKind.SERVER,
                    attributes: {
                        'http.method': method,
                        'http.target': req.url
                    }
                }, handleResponse));
        }
    } catch (err) {
        if (!(err instanceof _nofallbackerrorexternal.NoFallbackError)) {
            const silenceLog = false;
            await routeModule.onRequestError(req, err, {
                routerKind: 'App Router',
                routePath: normalizedSrcPage,
                routeType: 'route',
                revalidateReason: (0, _utils.getRevalidateReason)({
                    isStaticGeneration,
                    isOnDemandRevalidate
                })
            }, silenceLog, routerServerContext);
        }
        // rethrow so that we can handle serving error page
        // If this is during static generation, throw the error again.
        if (isIsr) throw err;
        // Otherwise, send a 500 response.
        await (0, _sendresponse.sendResponse)(nodeNextReq, nodeNextRes, new Response(null, {
            status: 500
        }));
        return null;
    }
}

//# sourceMappingURL=app-route.js.map