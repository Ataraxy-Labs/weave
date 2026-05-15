/**
 * This is a utility function to make scheduling sequential tasks that run back to back easier.
 * We schedule on the same queue (setTimeout) at the same time to ensure no other events can sneak in between.
 */
export declare function scheduleInSequentialTasks<R>(render: () => R | Promise<R>, followup: () => void): Promise<R>;
/**
 * This is a utility function to make scheduling sequential tasks that run back to back easier.
 * We schedule on the same queue (setTimeout) at the same time to ensure no other events can sneak in between.
 * The function that runs in the second task gets access to the first tasks's result.
 */
export declare function pipelineInSequentialTasks<A, B, C>(one: () => A, two: (a: A) => B, three: (b: B) => C): Promise<C>;
