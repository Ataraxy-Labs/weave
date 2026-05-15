import { loadEnvConfig } from '@next/env';
import * as inspector from 'inspector';
import * as Log from '../../build/output/log';
import { bold, purple, strikethrough } from '../../lib/picocolors';
import { PHASE_DEVELOPMENT_SERVER, PHASE_PRODUCTION_BUILD } from '../../shared/lib/constants';
import loadConfig from '../config';
import { experimentalSchema } from '../config-schema';
export function logStartInfo({ networkUrl, appUrl, envInfo, experimentalFeatures, logBundler, cacheComponents }) {
    let versionSuffix = '';
    const parts = [];
    if (logBundler) {
        if (process.env.TURBOPACK) {
            parts.push('Turbopack');
        } else if (process.env.NEXT_RSPACK) {
            parts.push('Rspack');
        } else {
            parts.push('webpack');
        }
    }
    if (cacheComponents) {
        parts.push('Cache Components');
    }
    if (parts.length > 0) {
        versionSuffix = ` (${parts.join(', ')})`;
    }
    Log.bootstrap(`${bold(purple(`${Log.prefixes.ready} Next.js ${"16.1.6"}`))}${versionSuffix}`);
    if (appUrl) {
        Log.bootstrap(`- Local:         ${appUrl}`);
    }
    if (networkUrl) {
        Log.bootstrap(`- Network:       ${networkUrl}`);
    }
    const inspectorUrl = inspector.url();
    if (inspectorUrl) {
        // Could also parse this port from the inspector URL.
        // process.debugPort will always be defined even if the process is not being inspected.
        // The full URL seems noisy as far as I can tell.
        // Node.js will print the full URL anyway.
        const debugPort = process.debugPort;
        Log.bootstrap(`- Debugger port: ${debugPort}`);
    }
    if (envInfo == null ? void 0 : envInfo.length) Log.bootstrap(`- Environments: ${envInfo.join(', ')}`);
    if (experimentalFeatures == null ? void 0 : experimentalFeatures.length) {
        Log.bootstrap(`- Experiments (use with caution):`);
        for (const exp of experimentalFeatures){
            const isValid = Object.prototype.hasOwnProperty.call(experimentalSchema, exp.key);
            if (isValid) {
                const symbol = typeof exp.value === 'boolean' ? exp.value === true ? bold('✓') : bold('⨯') : '·';
                const suffix = typeof exp.value === 'number' || typeof exp.value === 'string' ? `: ${JSON.stringify(exp.value)}` : '';
                const reason = exp.reason ? ` (${exp.reason})` : '';
                Log.bootstrap(`  ${symbol} ${exp.key}${suffix}${reason}`);
            } else {
                Log.bootstrap(`  ? ${strikethrough(exp.key)} (invalid experimental key)`);
            }
        }
    }
    // New line after the bootstrap info
    Log.info('');
}
export async function getStartServerInfo({ dir, dev, debugPrerender }) {
    let experimentalFeatures = [];
    let cacheComponents = false;
    const config = await loadConfig(dev ? PHASE_DEVELOPMENT_SERVER : PHASE_PRODUCTION_BUILD, dir, {
        reportExperimentalFeatures (features) {
            experimentalFeatures = features.sort(({ key: a }, { key: b })=>a.localeCompare(b));
        },
        debugPrerender,
        silent: false
    });
    cacheComponents = !!config.cacheComponents;
    // we need to reset env if we are going to create
    // the worker process with the esm loader so that the
    // initial env state is correct
    let envInfo = [];
    const { loadedEnvFiles } = loadEnvConfig(dir, true, console, false);
    if (loadedEnvFiles.length > 0) {
        envInfo = loadedEnvFiles.map((f)=>f.path);
    }
    return {
        envInfo,
        experimentalFeatures,
        cacheComponents
    };
}

//# sourceMappingURL=app-info-log.js.map