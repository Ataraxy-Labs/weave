import { extractInfoFromServerReferenceId } from '../shared/lib/server-reference-info';
export function isServerReference(value) {
    return value.$$typeof === Symbol.for('react.server.reference');
}
export function isUseCacheFunction(value) {
    if (!isServerReference(value)) {
        return false;
    }
    const { type } = extractInfoFromServerReferenceId(value.$$id);
    return type === 'use-cache';
}
export function getUseCacheFunctionInfo(value) {
    if (!isServerReference(value)) {
        return null;
    }
    const info = extractInfoFromServerReferenceId(value.$$id);
    return info.type === 'use-cache' ? info : null;
}
export function isClientReference(mod) {
    const defaultExport = (mod == null ? void 0 : mod.default) || mod;
    return (defaultExport == null ? void 0 : defaultExport.$$typeof) === Symbol.for('react.client.reference');
}

//# sourceMappingURL=client-and-server-references.js.map