import type { NextConfig } from '../../server/config-shared';
/**
 * Find the closest matching `quality` in the list of `config.qualities`
 * @param quality the quality prop passed to the image component
 * @param config the "images" configuration from next.config.js
 * @returns the closest matching quality value
 */
export declare function findClosestQuality(quality: number | undefined, config: NextConfig['images'] | undefined): number;
