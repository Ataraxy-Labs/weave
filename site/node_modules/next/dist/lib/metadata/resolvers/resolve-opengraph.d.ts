import type { ResolvedMetadataWithURLs } from '../types/metadata-interface';
import type { OpenGraph } from '../types/opengraph-types';
import type { FieldResolverExtraArgs, AsyncFieldResolverExtraArgs, MetadataContext } from '../types/resolvers';
import type { Twitter } from '../types/twitter-types';
import { type MetadataBaseURL } from './resolve-url';
export declare function resolveImages(images: Twitter['images'], metadataBase: MetadataBaseURL, isStaticMetadataRouteFile: boolean): NonNullable<ResolvedMetadataWithURLs['twitter']>['images'];
export declare function resolveImages(images: OpenGraph['images'], metadataBase: MetadataBaseURL, isStaticMetadataRouteFile: boolean): NonNullable<ResolvedMetadataWithURLs['openGraph']>['images'];
export declare const resolveOpenGraph: AsyncFieldResolverExtraArgs<'openGraph', [
    MetadataBaseURL,
    Promise<string>,
    MetadataContext,
    string | null
]>;
export declare const resolveTwitter: FieldResolverExtraArgs<'twitter', [
    MetadataBaseURL,
    MetadataContext,
    string | null
]>;
