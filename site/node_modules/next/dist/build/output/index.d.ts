import type { webpack } from 'next/dist/compiled/webpack/webpack';
export declare function watchCompilers(client: webpack.Compiler, server: webpack.Compiler, edgeServer: webpack.Compiler): void;
export declare function reportTrigger(trigger: string, url?: string): void;
