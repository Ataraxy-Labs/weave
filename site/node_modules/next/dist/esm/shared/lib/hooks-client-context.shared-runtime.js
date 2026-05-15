'use client';
import { createContext } from 'react';
import { ReadonlyURLSearchParams } from '../../client/components/readonly-url-search-params';
export const SearchParamsContext = createContext(null);
export const PathnameContext = createContext(null);
export const PathParamsContext = createContext(null);
export const NavigationPromisesContext = createContext(null);
// Creates an instrumented promise for Suspense DevTools
// These promises are always fulfilled and exist purely for
// tracking in React's Suspense DevTools.
export function createDevToolsInstrumentedPromise(displayName, value) {
    const promise = Promise.resolve(value);
    promise.status = 'fulfilled';
    promise.value = value;
    promise.displayName = `${displayName} (SSR)`;
    return promise;
}
export { ReadonlyURLSearchParams };
if (process.env.NODE_ENV !== 'production') {
    SearchParamsContext.displayName = 'SearchParamsContext';
    PathnameContext.displayName = 'PathnameContext';
    PathParamsContext.displayName = 'PathParamsContext';
    NavigationPromisesContext.displayName = 'NavigationPromisesContext';
}

//# sourceMappingURL=hooks-client-context.shared-runtime.js.map