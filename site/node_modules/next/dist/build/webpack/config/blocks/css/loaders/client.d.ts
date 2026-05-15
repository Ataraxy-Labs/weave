import type { webpack } from 'next/dist/compiled/webpack/webpack';
export declare function getClientStyleLoader({ hasAppDir, isAppDir, isDevelopment, assetPrefix, experimentalInlineCss, }: {
    hasAppDir: boolean;
    isAppDir?: boolean;
    isDevelopment: boolean;
    assetPrefix: string;
    experimentalInlineCss?: boolean;
}): webpack.RuleSetUseItem;
