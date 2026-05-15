import type { NextConfigComplete } from '../server/config-shared';
import loadConfig from '../server/config';
/**  */
export declare function validateTurboNextConfig({ dir, configPhase, }: {
    dir: string;
    configPhase: Parameters<typeof loadConfig>[0];
}): Promise<NextConfigComplete>;
