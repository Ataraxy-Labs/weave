import path from 'node:path';
import isError from '../../../lib/is-error';
import { INSTRUMENTATION_HOOK_FILENAME } from '../../../lib/constants';
import { interopDefault } from '../../../lib/interop-default';
import { afterRegistration as extendInstrumentationAfterRegistration } from './instrumentation-node-extensions';
let cachedInstrumentationModule;
export async function getInstrumentationModule(projectDir, distDir) {
    if (cachedInstrumentationModule) {
        return cachedInstrumentationModule;
    }
    try {
        cachedInstrumentationModule = interopDefault(await require(path.join(projectDir, distDir, 'server', `${INSTRUMENTATION_HOOK_FILENAME}.js`)));
        return cachedInstrumentationModule;
    } catch (err) {
        if (isError(err) && err.code !== 'ENOENT' && err.code !== 'MODULE_NOT_FOUND' && err.code !== 'ERR_MODULE_NOT_FOUND') {
            throw err;
        }
    }
}
let instrumentationModulePromise = null;
async function registerInstrumentation(projectDir, distDir) {
    // Ensure registerInstrumentation is not called in production build
    if (process.env.NEXT_PHASE === 'phase-production-build') {
        return;
    }
    if (!instrumentationModulePromise) {
        instrumentationModulePromise = getInstrumentationModule(projectDir, distDir);
    }
    const instrumentation = await instrumentationModulePromise;
    if (instrumentation == null ? void 0 : instrumentation.register) {
        try {
            await instrumentation.register();
            extendInstrumentationAfterRegistration();
        } catch (err) {
            err.message = `An error occurred while loading instrumentation hook: ${err.message}`;
            throw err;
        }
    }
}
export async function instrumentationOnRequestError(projectDir, distDir, ...args) {
    const instrumentation = await getInstrumentationModule(projectDir, distDir);
    try {
        var _instrumentation_onRequestError;
        await (instrumentation == null ? void 0 : (_instrumentation_onRequestError = instrumentation.onRequestError) == null ? void 0 : _instrumentation_onRequestError.call(instrumentation, ...args));
    } catch (err) {
        // Log the soft error and continue, since the original error has already been thrown
        console.error('Error in instrumentation.onRequestError:', err);
    }
}
let registerInstrumentationPromise = null;
export function ensureInstrumentationRegistered(projectDir, distDir) {
    if (!registerInstrumentationPromise) {
        registerInstrumentationPromise = registerInstrumentation(projectDir, distDir);
    }
    return registerInstrumentationPromise;
}

//# sourceMappingURL=instrumentation-globals.external.js.map