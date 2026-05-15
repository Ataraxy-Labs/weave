import type { FocusAndScrollRef, PrefetchKind } from '../../client/components/router-reducer/router-reducer-types';
import type { Params } from '../../server/request/params';
import type { FlightRouterState, FlightSegmentPath, CacheNode } from './app-router-types';
import React from 'react';
export interface NavigateOptions {
    scroll?: boolean;
}
export interface PrefetchOptions {
    kind: PrefetchKind;
    onInvalidate?: () => void;
}
export interface AppRouterInstance {
    /**
     * Navigate to the previous history entry.
     */
    back(): void;
    /**
     * Navigate to the next history entry.
     */
    forward(): void;
    /**
     * Refresh the current page.
     */
    refresh(): void;
    /**
     * Navigate to the provided href.
     * Pushes a new history entry.
     */
    push(href: string, options?: NavigateOptions): void;
    /**
     * Navigate to the provided href.
     * Replaces the current history entry.
     */
    replace(href: string, options?: NavigateOptions): void;
    /**
     * Prefetch the provided href.
     */
    prefetch(href: string, options?: PrefetchOptions): void;
}
export declare const AppRouterContext: React.Context<AppRouterInstance | null>;
export declare const LayoutRouterContext: React.Context<{
    parentTree: FlightRouterState;
    parentCacheNode: CacheNode;
    parentSegmentPath: FlightSegmentPath | null;
    parentParams: Params;
    debugNameContext: string;
    url: string;
    isActive: boolean;
} | null>;
export declare const GlobalLayoutRouterContext: React.Context<{
    tree: FlightRouterState;
    focusAndScrollRef: FocusAndScrollRef;
    nextUrl: string | null;
    previousNextUrl: string | null;
}>;
export declare const TemplateContext: React.Context<React.ReactNode>;
export declare const MissingSlotContext: React.Context<Set<string>>;
