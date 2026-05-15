export function createHrefFromUrl(url, includeHash = true) {
    return url.pathname + url.search + (includeHash ? url.hash : '');
}

//# sourceMappingURL=create-href-from-url.js.map