import type { TurbopackInternalErrorOpts } from '../../../build/swc/generated-native';
/**
 * An error caused by a bug in Turbopack, and not the user's code (e.g. a Rust panic). These should
 * be written to a log file and details should not be shown to the user.
 *
 * These are constructed in Turbopack by calling `throwTurbopackInternalError`.
 */
export declare class TurbopackInternalError extends Error {
    name: string;
    location: string | undefined;
    __NEXT_ERROR_CODE: string;
    constructor({ message, anonymizedLocation }: TurbopackInternalErrorOpts);
}
/**
 * A helper used by the napi Rust entrypoints to construct and throw a `TurbopackInternalError`.
 *
 * When called, this will emit a telemetry event.
 */
export declare function throwTurbopackInternalError(conversionError: Error | null, opts: TurbopackInternalErrorOpts): never;
