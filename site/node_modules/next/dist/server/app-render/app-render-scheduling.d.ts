/**
 * Allows scheduling multiple timers (equivalent to `setTimeout(cb, delayMs)`)
 * that are guaranteed to run in the same iteration of the event loop.
 *
 * @param delayMs - the delay to pass to `setTimeout`. (default: 0)
 *
 * */
export declare function createAtomicTimerGroup(delayMs?: number): (callback: () => void) => NodeJS.Timeout;
