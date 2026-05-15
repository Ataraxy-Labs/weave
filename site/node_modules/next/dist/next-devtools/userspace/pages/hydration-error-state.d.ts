import type { HydrationErrorState } from '../../shared/hydration-error';
export declare function getSquashedHydrationErrorDetails(error: Error): HydrationErrorState | null;
export declare function attachHydrationErrorState(error: Error): void;
export declare function storeHydrationErrorStateFromConsoleArgs(...args: any[]): void;
