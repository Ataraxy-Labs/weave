import type { UnknownMapEntry } from './cache-map';
export declare function lruPut(node: UnknownMapEntry): void;
export declare function updateLruSize(node: UnknownMapEntry, newNodeSize: number): void;
export declare function deleteFromLru(deleted: UnknownMapEntry): void;
