import { ReadonlyURLSearchParams } from './readonly-url-search-params';
export function unstable_isUnrecognizedActionError() {
    throw Object.defineProperty(new Error('`unstable_isUnrecognizedActionError` can only be used on the client.'), "__NEXT_ERROR_CODE", {
        value: "E776",
        enumerable: false,
        configurable: true
    });
}
export { redirect, permanentRedirect } from './redirect';
export { RedirectType } from './redirect-error';
export { notFound } from './not-found';
export { forbidden } from './forbidden';
export { unauthorized } from './unauthorized';
export { unstable_rethrow } from './unstable-rethrow';
export { ReadonlyURLSearchParams };

//# sourceMappingURL=navigation.react-server.js.map