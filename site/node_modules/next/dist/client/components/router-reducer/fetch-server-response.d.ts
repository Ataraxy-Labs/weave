import type { FlightRouterState } from '../../../shared/lib/app-router-types';
import { type NEXT_ROUTER_PREFETCH_HEADER, type NEXT_ROUTER_SEGMENT_PREFETCH_HEADER, NEXT_ROUTER_STATE_TREE_HEADER, NEXT_URL, RSC_HEADER, NEXT_HMR_REFRESH_HEADER, NEXT_HTML_REQUEST_ID_HEADER, NEXT_REQUEST_ID_HEADER } from '../app-router-headers';
import { type NormalizedFlightData } from '../../flight-data-helpers';
import type { NormalizedSearch } from '../segment-cache/cache-key';
export interface FetchServerResponseOptions {
    readonly flightRouterState: FlightRouterState;
    readonly nextUrl: string | null;
    readonly isHmrRefresh?: boolean;
}
type SpaFetchServerResponseResult = {
    flightData: NormalizedFlightData[];
    canonicalUrl: URL;
    renderedSearch: NormalizedSearch;
    couldBeIntercepted: boolean;
    prerendered: boolean;
    postponed: boolean;
    staleTime: number;
    debugInfo: Array<any> | null;
};
type MpaFetchServerResponseResult = string;
export type FetchServerResponseResult = MpaFetchServerResponseResult | SpaFetchServerResponseResult;
export type RequestHeaders = {
    [RSC_HEADER]?: '1';
    [NEXT_ROUTER_STATE_TREE_HEADER]?: string;
    [NEXT_URL]?: string;
    [NEXT_ROUTER_PREFETCH_HEADER]?: '1' | '2';
    [NEXT_ROUTER_SEGMENT_PREFETCH_HEADER]?: string;
    'x-deployment-id'?: string;
    [NEXT_HMR_REFRESH_HEADER]?: '1';
    'Next-Test-Fetch-Priority'?: RequestInit['priority'];
    [NEXT_HTML_REQUEST_ID_HEADER]?: string;
    [NEXT_REQUEST_ID_HEADER]?: string;
};
/**
 * Fetch the flight data for the provided url. Takes in the current router state
 * to decide what to render server-side.
 */
export declare function fetchServerResponse(url: URL, options: FetchServerResponseOptions): Promise<FetchServerResponseResult>;
export type RSCResponse<T> = {
    ok: boolean;
    redirected: boolean;
    headers: Headers;
    body: ReadableStream<Uint8Array> | null;
    status: number;
    url: string;
    flightResponse: (Promise<T> & {
        _debugInfo?: Array<any>;
    }) | null;
};
export declare function createFetch<T>(url: URL, headers: RequestHeaders, fetchPriority: 'auto' | 'high' | 'low' | null, shouldImmediatelyDecode: boolean, signal?: AbortSignal): Promise<RSCResponse<T>>;
export declare function createFromNextReadableStream<T>(flightStream: ReadableStream<Uint8Array>, requestHeaders: RequestHeaders): Promise<T>;
export {};
