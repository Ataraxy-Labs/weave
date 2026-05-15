import type { SizeLimit } from '../../types';
export declare const DEFAULT_MAX_POSTPONED_STATE_SIZE: SizeLimit;
/**
 * Parses the maxPostponedStateSize config value, using the default if not provided.
 */
export declare function parseMaxPostponedStateSize(size: SizeLimit | undefined): number | undefined;
