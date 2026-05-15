import type { Timestamp } from '../cache-handlers/types';
export interface TagManifestEntry {
    stale?: number;
    expired?: number;
}
export declare const tagsManifest: Map<string, TagManifestEntry>;
export declare const areTagsExpired: (tags: string[], timestamp: Timestamp) => boolean;
export declare const areTagsStale: (tags: string[], timestamp: Timestamp) => boolean;
