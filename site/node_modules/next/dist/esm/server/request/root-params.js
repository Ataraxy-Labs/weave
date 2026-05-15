import { InvariantError } from '../../shared/lib/invariant-error';
import { postponeWithTracking, throwToInterruptStaticGeneration } from '../app-render/dynamic-rendering';
import { workAsyncStorage } from '../app-render/work-async-storage.external';
import { workUnitAsyncStorage } from '../app-render/work-unit-async-storage.external';
import { makeHangingPromise } from '../dynamic-rendering-utils';
import { describeStringPropertyAccess } from '../../shared/lib/utils/reflect-utils';
import { actionAsyncStorage } from '../app-render/action-async-storage.external';
/**
 * Used for the compiler-generated `next/root-params` module.
 * @internal
 */ export function getRootParam(paramName) {
    const apiName = `\`import('next/root-params').${paramName}()\``;
    const workStore = workAsyncStorage.getStore();
    if (!workStore) {
        throw Object.defineProperty(new InvariantError(`Missing workStore in ${apiName}`), "__NEXT_ERROR_CODE", {
            value: "E764",
            enumerable: false,
            configurable: true
        });
    }
    const workUnitStore = workUnitAsyncStorage.getStore();
    if (!workUnitStore) {
        throw Object.defineProperty(new Error(`Route ${workStore.route} used ${apiName} outside of a Server Component. This is not allowed.`), "__NEXT_ERROR_CODE", {
            value: "E774",
            enumerable: false,
            configurable: true
        });
    }
    const actionStore = actionAsyncStorage.getStore();
    if (actionStore) {
        if (actionStore.isAppRoute) {
            // TODO(root-params): add support for route handlers
            throw Object.defineProperty(new Error(`Route ${workStore.route} used ${apiName} inside a Route Handler. Support for this API in Route Handlers is planned for a future version of Next.js.`), "__NEXT_ERROR_CODE", {
                value: "E765",
                enumerable: false,
                configurable: true
            });
        }
        if (actionStore.isAction && workUnitStore.phase === 'action') {
            // Actions are not fundamentally tied to a route (even if they're always submitted from some page),
            // so root params would be inconsistent if an action is called from multiple roots.
            // Make sure we check if the phase is "action" - we should not error in the rerender
            // after an action revalidates or updates cookies (which will still have `actionStore.isAction === true`)
            throw Object.defineProperty(new Error(`${apiName} was used inside a Server Action. This is not supported. Functions from 'next/root-params' can only be called in the context of a route.`), "__NEXT_ERROR_CODE", {
                value: "E766",
                enumerable: false,
                configurable: true
            });
        }
    }
    switch(workUnitStore.type){
        case 'unstable-cache':
        case 'cache':
            {
                throw Object.defineProperty(new Error(`Route ${workStore.route} used ${apiName} inside \`"use cache"\` or \`unstable_cache\`. Support for this API inside cache scopes is planned for a future version of Next.js.`), "__NEXT_ERROR_CODE", {
                    value: "E760",
                    enumerable: false,
                    configurable: true
                });
            }
        case 'prerender':
        case 'prerender-client':
        case 'prerender-ppr':
        case 'prerender-legacy':
            {
                return createPrerenderRootParamPromise(paramName, workStore, workUnitStore, apiName);
            }
        case 'private-cache':
        case 'prerender-runtime':
        case 'request':
            {
                break;
            }
        default:
            {
                workUnitStore;
            }
    }
    return Promise.resolve(workUnitStore.rootParams[paramName]);
}
function createPrerenderRootParamPromise(paramName, workStore, prerenderStore, apiName) {
    switch(prerenderStore.type){
        case 'prerender-client':
            {
                throw Object.defineProperty(new InvariantError(`${apiName} must not be used within a client component. Next.js should be preventing ${apiName} from being included in client components statically, but did not in this case.`), "__NEXT_ERROR_CODE", {
                    value: "E693",
                    enumerable: false,
                    configurable: true
                });
            }
        case 'prerender':
        case 'prerender-legacy':
        case 'prerender-ppr':
        default:
    }
    const underlyingParams = prerenderStore.rootParams;
    switch(prerenderStore.type){
        case 'prerender':
            {
                // We are in a cacheComponents prerender.
                // The param is a fallback, so it should be treated as dynamic.
                if (prerenderStore.fallbackRouteParams && prerenderStore.fallbackRouteParams.has(paramName)) {
                    return makeHangingPromise(prerenderStore.renderSignal, workStore.route, apiName);
                }
                break;
            }
        case 'prerender-ppr':
            {
                // We aren't in a cacheComponents prerender, but the param is a fallback,
                // so we need to make an erroring params object which will postpone/error if you access it
                if (prerenderStore.fallbackRouteParams && prerenderStore.fallbackRouteParams.has(paramName)) {
                    return makeErroringRootParamPromise(paramName, workStore, prerenderStore, apiName);
                }
                break;
            }
        case 'prerender-legacy':
            {
                break;
            }
        default:
            {
                prerenderStore;
            }
    }
    // If the param is not a fallback param, we just return the statically available value.
    return Promise.resolve(underlyingParams[paramName]);
}
/** Deliberately async -- we want to create a rejected promise, not error synchronously. */ async function makeErroringRootParamPromise(paramName, workStore, prerenderStore, apiName) {
    const expression = describeStringPropertyAccess(apiName, paramName);
    // In most dynamic APIs, we also throw if `dynamic = "error"`.
    // However, root params are only dynamic when we're generating a fallback shell,
    // and even with `dynamic = "error"` we still support generating dynamic fallback shells.
    // TODO: remove this comment when cacheComponents is the default since there will be no `dynamic = "error"`
    switch(prerenderStore.type){
        case 'prerender-ppr':
            {
                return postponeWithTracking(workStore.route, expression, prerenderStore.dynamicTracking);
            }
        case 'prerender-legacy':
            {
                return throwToInterruptStaticGeneration(expression, workStore, prerenderStore);
            }
        default:
            {
                prerenderStore;
            }
    }
}

//# sourceMappingURL=root-params.js.map