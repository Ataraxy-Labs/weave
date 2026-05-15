import { eventErrorThrown } from '../../../telemetry/events';
import { traceGlobals } from '../../../trace/shared';
/**
 * An error caused by a bug in Turbopack, and not the user's code (e.g. a Rust panic). These should
 * be written to a log file and details should not be shown to the user.
 *
 * These are constructed in Turbopack by calling `throwTurbopackInternalError`.
 */ export class TurbopackInternalError extends Error {
    constructor({ message, anonymizedLocation }){
        super(message), this.name = 'TurbopackInternalError', // Manually set this as this isn't statically determinable
        this.__NEXT_ERROR_CODE = 'TurbopackInternalError';
        this.location = anonymizedLocation;
    }
}
/**
 * A helper used by the napi Rust entrypoints to construct and throw a `TurbopackInternalError`.
 *
 * When called, this will emit a telemetry event.
 */ export function throwTurbopackInternalError(conversionError, opts) {
    if (conversionError != null) {
        // Somehow napi failed to convert `opts` to a JS object??? Just give up and throw that instead.
        throw Object.defineProperty(new Error('NAPI type conversion error in throwTurbopackInternalError', {
            cause: conversionError
        }), "__NEXT_ERROR_CODE", {
            value: "E723",
            enumerable: false,
            configurable: true
        });
    }
    const err = new TurbopackInternalError(opts);
    const telemetry = traceGlobals.get('telemetry');
    if (telemetry) {
        telemetry.record(eventErrorThrown(err, opts.anonymizedLocation));
    } else {
        console.error('Expected `telemetry` to be set in globals');
    }
    throw err;
}

//# sourceMappingURL=internal-error.js.map