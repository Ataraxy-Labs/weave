/**
 * Converts a duration in seconds to a human-readable string format.
 * Formats duration based on magnitude for optimal readability:
 * - >= 2 minutes: show in minutes with 1 decimal place (e.g., "2.5min")
 * - >= 40 seconds: show in whole seconds (e.g., "45s")
 * - >= 2 seconds: show in seconds with 1 decimal place (e.g., "3.2s")
 * - < 2 seconds: show in milliseconds with 1 decimal place (e.g., "1500.0ms")
 *
 * @deprecated Use durationToStringWithNanoseconds instead, collect time in nanoseconds using process.hrtime.bigint().
 * @param compilerDuration - Duration in seconds as a number
 * @returns Formatted duration string with appropriate unit and precision
 */
export declare function durationToString(compilerDuration: number): string;
/**
 * Converts a high-resolution time tuple to seconds.
 *
 * @param hrtime - High-resolution time tuple of [seconds, nanoseconds]
 * @returns Duration in seconds as a floating-point number
 */
export declare function hrtimeToSeconds(hrtime: [number, number]): number;
/**
 * Converts a BigInt nanosecond duration to a human-readable string format.
 * This is the preferred method for formatting high-precision durations.
 *
 * @param hrtime - Duration in nanoseconds as a BigInt (typically from process.hrtime.bigint())
 * @returns Formatted duration string with appropriate unit and precision
 */
export declare function hrtimeBigIntDurationToString(hrtime: bigint): string;
/**
 * Converts a high-resolution time tuple to a human-readable string format.
 *
 * @deprecated Use hrtimeBigIntDurationToString with process.hrtime.bigint() for better precision.
 * @param hrtime - High-resolution time tuple of [seconds, nanoseconds]
 * @returns Formatted duration string with appropriate unit and precision
 */
export declare function hrtimeDurationToString(hrtime: [number, number]): string;
