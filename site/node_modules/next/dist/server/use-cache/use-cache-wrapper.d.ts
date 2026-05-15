import { type SearchParams } from '../request/search-params';
import type { Params } from '../request/params';
export interface UseCachePageProps {
    params: Promise<Params>;
    searchParams: Promise<SearchParams>;
    $$isPage: true;
}
export type UseCacheLayoutProps = {
    params: Promise<Params>;
    $$isLayout: true;
} & {
    [slot: string]: any;
};
export declare function cache(kind: string, id: string, boundArgsLength: number, originalFn: (...args: unknown[]) => Promise<unknown>, argsObj: IArguments): Promise<unknown>;
