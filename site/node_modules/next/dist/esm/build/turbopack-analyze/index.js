import path from 'path';
import { validateTurboNextConfig } from '../../lib/turbopack-warning';
import { isFileSystemCacheEnabledForBuild } from '../../shared/lib/turbopack/utils';
import { createDefineEnv, loadBindings } from '../swc';
import { isCI } from '../../server/ci-info';
import { backgroundLogCompilationEvents } from '../../shared/lib/turbopack/compilation-events';
import { getSupportedBrowsers } from '../utils';
import { normalizePath } from '../../lib/normalize-path';
import { PHASE_PRODUCTION_BUILD } from '../../shared/lib/constants';
export async function turbopackAnalyze(analyzeContext) {
    var _config_experimental, _config_turbopack, _config_turbopack1, _config_experimental1;
    await validateTurboNextConfig({
        dir: analyzeContext.dir,
        configPhase: PHASE_PRODUCTION_BUILD
    });
    const { config, dir, distDir, noMangling } = analyzeContext;
    const currentNodeJsVersion = process.versions.node;
    const startTime = process.hrtime();
    const bindings = await loadBindings(config == null ? void 0 : (_config_experimental = config.experimental) == null ? void 0 : _config_experimental.useWasmBinary);
    const dev = false;
    const supportedBrowsers = getSupportedBrowsers(dir, dev);
    const persistentCaching = isFileSystemCacheEnabledForBuild(config);
    const rootPath = ((_config_turbopack = config.turbopack) == null ? void 0 : _config_turbopack.root) || config.outputFileTracingRoot || dir;
    const project = await bindings.turbo.createProject({
        rootPath: ((_config_turbopack1 = config.turbopack) == null ? void 0 : _config_turbopack1.root) || config.outputFileTracingRoot || dir,
        projectPath: normalizePath(path.relative(rootPath, dir) || '.'),
        distDir,
        nextConfig: config,
        watch: {
            enable: false
        },
        dev,
        env: process.env,
        defineEnv: createDefineEnv({
            isTurbopack: true,
            config,
            dev,
            distDir,
            projectPath: dir,
            fetchCacheKeyPrefix: config.experimental.fetchCacheKeyPrefix,
            hasRewrites: false,
            // Implemented separately in Turbopack, doesn't have to be passed here.
            middlewareMatchers: undefined,
            rewrites: {
                beforeFiles: [],
                afterFiles: [],
                fallback: []
            }
        }),
        buildId: 'analyze-build',
        encryptionKey: '',
        previewProps: {
            previewModeId: '',
            previewModeEncryptionKey: '',
            previewModeSigningKey: ''
        },
        browserslistQuery: supportedBrowsers.join(', '),
        noMangling,
        writeRoutesHashesManifest: false,
        currentNodeJsVersion
    }, {
        persistentCaching,
        memoryLimit: (_config_experimental1 = config.experimental) == null ? void 0 : _config_experimental1.turbopackMemoryLimit,
        dependencyTracking: persistentCaching,
        isCi: isCI,
        isShortSession: true
    });
    try {
        backgroundLogCompilationEvents(project);
        await project.writeAnalyzeData(analyzeContext.appDirOnly);
        const shutdownPromise = project.shutdown();
        const time = process.hrtime(startTime);
        return {
            duration: time[0] + time[1] / 1e9,
            shutdownPromise
        };
    } catch (err) {
        await project.shutdown();
        throw err;
    }
}
let shutdownPromise;
export async function waitForShutdown() {
    if (shutdownPromise) {
        await shutdownPromise;
    }
}

//# sourceMappingURL=index.js.map