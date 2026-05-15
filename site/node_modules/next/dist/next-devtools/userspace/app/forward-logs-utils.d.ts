export declare const safeStringifyWithDepth: typeof import("safe-stable-stringify").stringify;
/**
 * allows us to:
 * - revive the undefined log in the server as it would look in the browser
 * - not read/attempt to serialize promises (next will console error if you do that, and will cause this program to infinitely recurse)
 * - if we read a proxy that throws (no way to detect if something is a proxy), explain to the user we can't read this data
 */
export declare function preLogSerializationClone<T>(value: T, seen?: WeakMap<WeakKey, any>): any;
export declare const logStringify: (data: unknown) => string;
