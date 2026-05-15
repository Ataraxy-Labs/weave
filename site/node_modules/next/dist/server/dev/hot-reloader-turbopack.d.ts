import type { NextJsHotReloaderInterface } from './hot-reloader-types';
import { type ServerFields, type SetupOpts } from '../lib/router-utils/setup-dev-bundler';
import type { Lockfile } from '../../build/lockfile';
export declare function createHotReloaderTurbopack(opts: SetupOpts & {
    isSrcDir: boolean;
}, serverFields: ServerFields, distDir: string, resetFetch: () => void, lockfile: Lockfile | undefined): Promise<NextJsHotReloaderInterface>;
