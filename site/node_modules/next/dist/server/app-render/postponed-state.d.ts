import type { OpaqueFallbackRouteParams } from '../../server/request/fallback-params';
import type { Params } from '../request/params';
import { type PrerenderResumeDataCache, type RenderResumeDataCache } from '../resume-data-cache/resume-data-cache';
export declare enum DynamicState {
    /**
     * The dynamic access occurred during the RSC render phase.
     */
    DATA = 1,
    /**
     * The dynamic access occurred during the HTML shell render phase.
     */
    HTML = 2
}
/**
 * The postponed state for dynamic data.
 */
export type DynamicDataPostponedState = {
    /**
     * The type of dynamic state.
     */
    readonly type: DynamicState.DATA;
    /**
     * The immutable resume data cache.
     */
    readonly renderResumeDataCache: RenderResumeDataCache;
};
/**
 * The postponed state for dynamic HTML.
 */
export type DynamicHTMLPostponedState = {
    /**
     * The type of dynamic state.
     */
    readonly type: DynamicState.HTML;
    /**
     * The postponed data used by React.
     */
    readonly data: [
        preludeState: DynamicHTMLPreludeState,
        postponed: ReactPostponed
    ];
    /**
     * The immutable resume data cache.
     */
    readonly renderResumeDataCache: RenderResumeDataCache;
};
export declare const enum DynamicHTMLPreludeState {
    Empty = 0,
    Full = 1
}
type ReactPostponed = NonNullable<import('react-dom/static').PrerenderResult['postponed']>;
export type PostponedState = DynamicDataPostponedState | DynamicHTMLPostponedState;
export declare function getDynamicHTMLPostponedState(postponed: ReactPostponed, preludeState: DynamicHTMLPreludeState, fallbackRouteParams: OpaqueFallbackRouteParams | null, resumeDataCache: PrerenderResumeDataCache | RenderResumeDataCache, isCacheComponentsEnabled: boolean): Promise<string>;
export declare function getDynamicDataPostponedState(resumeDataCache: PrerenderResumeDataCache | RenderResumeDataCache, isCacheComponentsEnabled: boolean): Promise<string>;
export declare function parsePostponedState(state: string, interpolatedParams: Params, maxPostponedStateSizeBytes: number | undefined): PostponedState;
export declare function getPostponedFromState(state: DynamicHTMLPostponedState): {
    preludeState: DynamicHTMLPreludeState;
    postponed: import("react-dom/static").PostponedState;
};
export {};
