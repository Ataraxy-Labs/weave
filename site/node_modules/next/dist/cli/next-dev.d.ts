#!/usr/bin/env node
import '../server/lib/cpu-profile';
import { type DebugAddress } from '../server/lib/utils';
export type NextDevOptions = {
    disableSourceMaps: boolean;
    inspect?: DebugAddress | true;
    turbo?: boolean;
    turbopack?: boolean;
    webpack?: boolean;
    port: number;
    hostname?: string;
    experimentalHttps?: boolean;
    experimentalHttpsKey?: string;
    experimentalHttpsCert?: string;
    experimentalHttpsCa?: string;
    experimentalUploadTrace?: string;
    experimentalNextConfigStripTypes?: boolean;
};
type PortSource = 'cli' | 'default' | 'env';
declare const nextDev: (options: NextDevOptions, portSource: PortSource, directory?: string) => Promise<void>;
export { nextDev };
