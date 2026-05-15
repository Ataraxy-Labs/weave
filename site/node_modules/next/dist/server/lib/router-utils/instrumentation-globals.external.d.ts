import type { InstrumentationModule, InstrumentationOnRequestError } from '../../instrumentation/types';
export declare function getInstrumentationModule(projectDir: string, distDir: string): Promise<InstrumentationModule | undefined>;
export declare function instrumentationOnRequestError(projectDir: string, distDir: string, ...args: Parameters<InstrumentationOnRequestError>): Promise<void>;
export declare function ensureInstrumentationRegistered(projectDir: string, distDir: string): Promise<void>;
