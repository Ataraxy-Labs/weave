export declare enum RenderStage {
    Before = 1,
    Static = 2,
    Runtime = 3,
    Dynamic = 4,
    Abandoned = 5
}
export type NonStaticRenderStage = RenderStage.Runtime | RenderStage.Dynamic;
export declare class StagedRenderingController {
    private abortSignal;
    private hasRuntimePrefetch;
    currentStage: RenderStage;
    staticInterruptReason: Error | null;
    runtimeInterruptReason: Error | null;
    staticStageEndTime: number;
    runtimeStageEndTime: number;
    private runtimeStageListeners;
    private dynamicStageListeners;
    private runtimeStagePromise;
    private dynamicStagePromise;
    private mayAbandon;
    constructor(abortSignal: (AbortSignal | null) | undefined, hasRuntimePrefetch: boolean);
    onStage(stage: NonStaticRenderStage, callback: () => void): void;
    canSyncInterrupt(): boolean;
    syncInterruptCurrentStageWithReason(reason: Error): void;
    getStaticInterruptReason(): Error | null;
    getRuntimeInterruptReason(): Error | null;
    getStaticStageEndTime(): number;
    getRuntimeStageEndTime(): number;
    abandonRender(): void;
    private abandonRenderImpl;
    advanceStage(stage: RenderStage.Static | RenderStage.Runtime | RenderStage.Dynamic): void;
    /** Fire the `onStage` listeners for the runtime stage and unblock any promises waiting for it. */
    private resolveRuntimeStage;
    /** Fire the `onStage` listeners for the dynamic stage and unblock any promises waiting for it. */
    private resolveDynamicStage;
    private getStagePromise;
    waitForStage(stage: NonStaticRenderStage): Promise<void>;
    delayUntilStage<T>(stage: NonStaticRenderStage, displayName: string | undefined, resolvedValue: T): Promise<T>;
}
