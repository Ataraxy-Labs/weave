import type { NextConfigComplete } from '../server/config-shared';
import type { PageStaticInfo } from './analysis/get-page-static-info';
import type { PageExtensions } from './page-extensions-type';
export declare function getStaticInfoIncludingLayouts({ isInsideAppDir, pageExtensions, pageFilePath, appDir, config: nextConfig, isDev, page, }: {
    isInsideAppDir: boolean;
    pageExtensions: PageExtensions;
    pageFilePath: string;
    appDir: string | undefined;
    config: NextConfigComplete;
    isDev: boolean;
    page: string;
}): Promise<PageStaticInfo>;
