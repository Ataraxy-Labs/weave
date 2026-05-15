"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "transpileConfig", {
    enumerable: true,
    get: function() {
        return transpileConfig;
    }
});
const _nodepath = require("node:path");
const _promises = require("node:fs/promises");
const _nodeurl = require("node:url");
const _requirehook = require("./require-hook");
const _log = require("../output/log");
const _installdependencies = require("../../lib/install-dependencies");
const _utils = require("../../server/lib/utils");
function resolveSWCOptions(cwd, compilerOptions) {
    var _process_versions, _process;
    return {
        jsc: {
            parser: {
                syntax: 'typescript'
            },
            ...compilerOptions.paths ? {
                paths: compilerOptions.paths
            } : {},
            ...compilerOptions.baseUrl ? {
                baseUrl: (0, _nodepath.resolve)(cwd, compilerOptions.baseUrl)
            } : compilerOptions.paths ? {
                baseUrl: cwd
            } : {}
        },
        module: {
            type: 'commonjs'
        },
        isModule: 'unknown',
        env: {
            targets: {
                // Setting the Node.js version can reduce unnecessary code generation.
                node: ((_process = process) == null ? void 0 : (_process_versions = _process.versions) == null ? void 0 : _process_versions.node) ?? '20.19.0'
            }
        }
    };
}
// Ported from next/src/lib/verify-typescript-setup.ts
// Although this overlaps with the later `verifyTypeScriptSetup`,
// it is acceptable since the time difference in the worst case is trivial,
// as we are only preparing to install the dependencies once more.
async function verifyTypeScriptSetup(cwd, configFileName) {
    try {
        // Quick module check.
        require.resolve('typescript', {
            paths: [
                cwd
            ]
        });
    } catch (error) {
        if (error && typeof error === 'object' && 'code' in error && error.code === 'MODULE_NOT_FOUND') {
            (0, _log.warn)(`Installing TypeScript as it was not found while loading "${configFileName}".`);
            await (0, _installdependencies.installDependencies)(cwd, [
                {
                    pkg: 'typescript'
                }
            ], true).catch((err)=>{
                if (err && typeof err === 'object' && 'command' in err) {
                    console.error(`Failed to install TypeScript, please install it manually to continue:\n` + err.command + '\n');
                }
                throw err;
            });
        }
    }
}
async function getTsConfig(cwd) {
    const ts = require(require.resolve('typescript', {
        paths: [
            cwd
        ]
    }));
    // NOTE: This doesn't fully cover the edge case for setting
    // "typescript.tsconfigPath" in next config which is currently
    // a restriction.
    const tsConfigPath = ts.findConfigFile(cwd, ts.sys.fileExists, 'tsconfig.json');
    if (!tsConfigPath) {
        // It is ok to not return ts.getDefaultCompilerOptions() because
        // we are only looking for paths and baseUrl from tsConfig.
        return {};
    }
    const configFile = ts.readConfigFile(tsConfigPath, ts.sys.readFile);
    const parsedCommandLine = ts.parseJsonConfigFileContent(configFile.config, ts.sys, cwd);
    return parsedCommandLine.options;
}
async function transpileConfig({ nextConfigPath, configFileName, cwd }) {
    try {
        // envs are passed to the workers and preserve the flag
        if (process.env.__NEXT_NODE_NATIVE_TS_LOADER_ENABLED === 'true') {
            try {
                // Node.js v22.10.0+
                // Value is 'strip' or 'transform' based on how the feature is enabled.
                // https://nodejs.org/api/process.html#processfeaturestypescript
                // TODO: Remove `as any` once we bump @types/node to v22.10.0+
                if (process.features.typescript) {
                    // Run import() here to catch errors and fallback to legacy resolution.
                    return (await import((0, _nodeurl.pathToFileURL)(nextConfigPath).href)).default;
                }
                if ((0, _utils.getNodeOptionsArgs)().includes('--no-experimental-strip-types') || process.execArgv.includes('--no-experimental-strip-types')) {
                    (0, _log.warnOnce)(`Skipped resolving "${configFileName}" using Node.js native TypeScript resolution because it was disabled by the "--no-experimental-strip-types" flag.` + ' Falling back to legacy resolution.' + ' Learn more: https://nextjs.org/docs/app/api-reference/config/typescript#using-nodejs-native-typescript-resolver-for-nextconfigts');
                }
                // Feature is not enabled, fallback to legacy resolution for current session.
                process.env.__NEXT_NODE_NATIVE_TS_LOADER_ENABLED = 'false';
            } catch (cause) {
                (0, _log.warnOnce)(`Failed to import "${configFileName}" using Node.js native TypeScript resolution.` + ' Falling back to legacy resolution.' + ' Learn more: https://nextjs.org/docs/app/api-reference/config/typescript#using-nodejs-native-typescript-resolver-for-nextconfigts', {
                    cause
                });
                // Once failed, fallback to legacy resolution for current session.
                process.env.__NEXT_NODE_NATIVE_TS_LOADER_ENABLED = 'false';
            }
        }
        // Ensure TypeScript is installed to use the API.
        await verifyTypeScriptSetup(cwd, configFileName);
        const compilerOptions = await getTsConfig(cwd);
        return handleCJS({
            cwd,
            nextConfigPath,
            compilerOptions
        });
    } catch (cause) {
        throw Object.defineProperty(new Error(`Failed to transpile "${configFileName}".`, {
            cause
        }), "__NEXT_ERROR_CODE", {
            value: "E797",
            enumerable: false,
            configurable: true
        });
    }
}
async function handleCJS({ cwd, nextConfigPath, compilerOptions }) {
    const swcOptions = resolveSWCOptions(cwd, compilerOptions);
    let hasRequire = false;
    try {
        var _config_experimental;
        const nextConfigString = await (0, _promises.readFile)(nextConfigPath, 'utf8');
        // lazy require swc since it loads React before even setting NODE_ENV
        // resulting loading Development React on Production
        const { loadBindings } = require('../swc');
        const bindings = await loadBindings();
        const { code } = await bindings.transform(nextConfigString, swcOptions);
        // register require hook only if require exists
        if (code.includes('require(')) {
            (0, _requirehook.registerHook)(swcOptions);
            hasRequire = true;
        }
        // filename & extension don't matter here
        const config = (0, _requirehook.requireFromString)(code, (0, _nodepath.resolve)(cwd, 'next.config.compiled.js'));
        // At this point we have already loaded the bindings without this configuration setting due to the `transform` call above.
        // Possibly we fell back to wasm in which case, it all works out but if not we need to warn
        // that the configuration was ignored.
        if ((config == null ? void 0 : (_config_experimental = config.experimental) == null ? void 0 : _config_experimental.useWasmBinary) && !bindings.isWasm) {
            (0, _log.warn)('Using a next.config.ts file is incompatible with `experimental.useWasmBinary` unless ' + '`--experimental-next-config-strip-types` is also passed.\nSetting `useWasmBinary` to `false');
            config.experimental.useWasmBinary = false;
        }
        return config;
    } catch (error) {
        throw error;
    } finally{
        if (hasRequire) {
            (0, _requirehook.deregisterHook)();
        }
    }
}

//# sourceMappingURL=transpile-config.js.map