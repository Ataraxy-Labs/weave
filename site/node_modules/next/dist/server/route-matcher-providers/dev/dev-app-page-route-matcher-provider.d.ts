import type { FileReader } from './helpers/file-reader/file-reader';
import { AppPageRouteMatcher } from '../../route-matchers/app-page-route-matcher';
import { FileCacheRouteMatcherProvider } from './file-cache-route-matcher-provider';
export declare class DevAppPageRouteMatcherProvider extends FileCacheRouteMatcherProvider<AppPageRouteMatcher> {
    private readonly expression;
    private readonly normalizers;
    private readonly isTurbopack;
    constructor(appDir: string, extensions: ReadonlyArray<string>, reader: FileReader, isTurbopack: boolean);
    protected transform(files: ReadonlyArray<string>): Promise<ReadonlyArray<AppPageRouteMatcher>>;
}
