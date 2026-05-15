/// Utilties for configuring the bundler to use.
export var Bundler = /*#__PURE__*/ function(Bundler) {
    Bundler[Bundler["Turbopack"] = 0] = "Turbopack";
    Bundler[Bundler["Webpack"] = 1] = "Webpack";
    Bundler[Bundler["Rspack"] = 2] = "Rspack";
    return Bundler;
}({});
/**
 * Parse the bundler arguments and potentially sets the `TURBOPACK` environment variable.
 *
 * NOTE: rspack is configured via next config which is chaotic so it is possible for this to be overridden later.
 *
 * @param options The options to parse.
 * @returns The bundler that was configured
 */ export function parseBundlerArgs(options) {
    const bundlerFlags = new Map();
    const setBundlerFlag = (bundler, flag)=>{
        bundlerFlags.set(bundler, (bundlerFlags.get(bundler) ?? []).concat(flag));
    };
    // What turbo flag was set? We allow multiple to be set, which is silly but not ambiguous, just pick the most relevant one.
    if (options.turbopack) {
        setBundlerFlag(0, '--turbopack');
    }
    if (options.turbo) {
        setBundlerFlag(0, '--turbo');
    } else if (process.env.TURBOPACK) {
        // We don't really want to support this but it is trivial and not really confusing.
        // If we don't support it and someone sets it, we would have inconsistent behavior
        // since some parts of next would read the return value of this function and other
        // parts will read the env variable.
        setBundlerFlag(0, `TURBOPACK=${process.env.TURBOPACK}`);
    } else if (process.env.IS_TURBOPACK_TEST) {
        setBundlerFlag(0, `IS_TURBOPACK_TEST=${process.env.IS_TURBOPACK_TEST}`);
    }
    if (options.webpack) {
        setBundlerFlag(1, '--webpack');
    }
    if (process.env.IS_WEBPACK_TEST) {
        setBundlerFlag(1, `IS_WEBPACK_TEST=${process.env.IS_WEBPACK_TEST}`);
    }
    // Mostly this is set via the NextConfig but it can also be set via the command line which is
    // common for testing.
    if (process.env.NEXT_RSPACK) {
        setBundlerFlag(2, `NEXT_RSPACK=${process.env.NEXT_RSPACK}`);
    }
    if (process.env.NEXT_TEST_USE_RSPACK) {
        setBundlerFlag(2, `NEXT_TEST_USE_RSPACK=${process.env.NEXT_TEST_USE_RSPACK}`);
    }
    if (bundlerFlags.size > 1) {
        console.error(`Multiple bundler flags set: ${Array.from(bundlerFlags.values()).flat().join(', ')}.

Edit your command or your package.json script to configure only one bundler.`);
        process.exit(1);
    }
    // The default is turbopack when nothing is configured.
    if (bundlerFlags.size === 0) {
        process.env.TURBOPACK = 'auto';
        return 0;
    }
    if (bundlerFlags.has(0)) {
        // Only conditionally assign to the environment variable, preserving already set values.
        // If it was set to 'auto' because no flag was set and this function is called a second time we
        // would upgrade to '1' but we don't really want that.
        process.env.TURBOPACK ??= '1';
        return 0;
    }
    // Otherwise it is one of rspack or webpack. At this point there must be exactly one key in the map.
    return bundlerFlags.keys().next().value;
}
/**
 * Finalize the bundler based on the config.
 *
 * Rspack is configured via next config by setting an environment variable (yay, side effects)
 * so this should only be called after parsing the config.
 */ export function finalizeBundlerFromConfig(fromOptions) {
    // Reading the next config can set NEXT_RSPACK environment variables.
    if (process.env.NEXT_RSPACK) {
        return 2;
    }
    return fromOptions;
}

//# sourceMappingURL=bundler.js.map