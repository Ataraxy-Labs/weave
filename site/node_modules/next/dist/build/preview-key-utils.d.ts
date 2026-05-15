import type { __ApiPreviewProps } from '../server/api-utils';
export declare function generatePreviewKeys({ distDir, isBuild, }: {
    distDir: string;
    isBuild: boolean;
}): Promise<__ApiPreviewProps>;
