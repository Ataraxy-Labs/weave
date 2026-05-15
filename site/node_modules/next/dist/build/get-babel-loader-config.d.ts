import type { ReactCompilerOptions } from '../server/config-shared';
declare const getBabelLoader: (useSWCLoader: boolean | undefined, babelConfigFile: string | undefined, isServer: boolean, distDir: string, pagesDir: string | undefined, cwd: string, srcDir: string, dev: boolean, isClient: boolean, reactCompilerOptions: boolean | ReactCompilerOptions | undefined, reactCompilerExclude: ((excludePath: string) => boolean) | undefined) => {
    loader: string;
    options: import("./babel/loader/types").NextBabelLoaderOptionDefaultPresets;
} | undefined;
/**
 * Get a separate babel loader for the react compiler, only used if Babel is not
 * configured through e.g. .babelrc. If user have babel config, this should be configured in the babel loader itself.
 * Note from react compiler:
 * > For best results, compiler must run as the first plugin in your Babel pipeline so it receives input as close to the original source as possible.
 */
declare const getReactCompilerLoader: (reactCompilerOptions: boolean | ReactCompilerOptions | undefined, cwd: string, isServer: boolean, reactCompilerExclude: ((excludePath: string) => boolean) | undefined, isDev: boolean) => {
    loader: string;
    options: import("./babel/loader/types").NextBabelLoaderOptionStandalone;
} | undefined;
export { getBabelLoader, getReactCompilerLoader };
