import { type WorkStore } from '../app-render/work-async-storage.external';
export type ParamValue = string | Array<string> | undefined;
export type Params = Record<string, ParamValue>;
export declare function createParamsFromClient(underlyingParams: Params, workStore: WorkStore): Promise<Params>;
export type CreateServerParamsForMetadata = typeof createServerParamsForMetadata;
export declare const createServerParamsForMetadata: typeof createServerParamsForServerSegment;
export declare function createServerParamsForRoute(underlyingParams: Params, workStore: WorkStore): Promise<Params>;
export declare function createServerParamsForServerSegment(underlyingParams: Params, workStore: WorkStore): Promise<Params>;
export declare function createPrerenderParamsForClientSegment(underlyingParams: Params): Promise<Params>;
