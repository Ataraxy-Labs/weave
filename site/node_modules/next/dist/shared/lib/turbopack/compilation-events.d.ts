import type { Project } from '../../../build/swc/types';
/**
 * Subscribes to compilation events for `project` and prints them using the
 * `Log` library.
 *
 * The `signal` argument is partially implemented. The abort may not happen until the next
 * compilation event arrives.
 */
export declare function backgroundLogCompilationEvents(project: Project, { eventTypes, signal }?: {
    eventTypes?: string[];
    signal?: AbortSignal;
}): void;
