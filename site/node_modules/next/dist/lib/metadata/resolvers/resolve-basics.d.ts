import type { Viewport } from '../types/metadata-interface';
import type { FieldResolver, AsyncFieldResolverExtraArgs, MetadataContext } from '../types/resolvers';
import { type MetadataBaseURL } from './resolve-url';
export declare const resolveThemeColor: FieldResolver<'themeColor', Viewport>;
export declare const resolveAlternates: AsyncFieldResolverExtraArgs<'alternates', [
    MetadataBaseURL,
    Promise<string>,
    MetadataContext
]>;
export declare const resolveRobots: FieldResolver<'robots'>;
export declare const resolveVerification: FieldResolver<'verification'>;
export declare const resolveAppleWebApp: FieldResolver<'appleWebApp'>;
export declare const resolveAppLinks: FieldResolver<'appLinks'>;
export declare const resolveItunes: AsyncFieldResolverExtraArgs<'itunes', [
    MetadataBaseURL,
    Promise<string>,
    MetadataContext
]>;
export declare const resolveFacebook: FieldResolver<'facebook'>;
export declare const resolvePagination: AsyncFieldResolverExtraArgs<'pagination', [
    MetadataBaseURL,
    Promise<string>,
    MetadataContext
]>;
