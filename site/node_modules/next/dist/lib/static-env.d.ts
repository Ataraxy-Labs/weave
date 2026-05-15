import type { NextConfigComplete, NextConfigRuntime } from '../server/config-shared';
/**
 * Collects all environment variables that are using the `NEXT_PUBLIC_` prefix.
 */
export declare function getNextPublicEnvironmentVariables(): {
    [k: string]: string | undefined;
};
/**
 * Collects the `env` config value from the Next.js config.
 */
export declare function getNextConfigEnv(config: NextConfigComplete | NextConfigRuntime): Record<string, string | undefined>;
export declare function getStaticEnv(config: NextConfigComplete | NextConfigRuntime, deploymentId: string): Record<string, string | undefined>;
export declare function populateStaticEnv(config: NextConfigComplete | NextConfigRuntime, deploymentId: string): void;
