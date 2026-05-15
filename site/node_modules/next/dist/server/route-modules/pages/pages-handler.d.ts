import type { IncomingMessage, ServerResponse } from 'node:http';
import type { PagesRouteModule } from './module.compiled';
import type { GetServerSideProps, GetStaticPaths, GetStaticProps } from '../../../types';
export declare const getHandler: ({ srcPage: originalSrcPage, config, userland, routeModule, isFallbackError, getStaticPaths, getStaticProps, getServerSideProps, }: {
    srcPage: string;
    config: Record<string, any> | undefined;
    userland: any;
    isFallbackError?: boolean;
    routeModule: PagesRouteModule;
    getStaticProps?: GetStaticProps;
    getStaticPaths?: GetStaticPaths;
    getServerSideProps?: GetServerSideProps;
}) => (req: IncomingMessage, res: ServerResponse, ctx: {
    waitUntil: (prom: Promise<void>) => void;
}) => Promise<void>;
