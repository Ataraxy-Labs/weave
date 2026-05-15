import type { VersionInfo } from '../../server/dev/parse-version-info';
export declare function getStaleness({ installed, staleness, expected }: VersionInfo): {
    text: string;
    indicatorClass: string;
    title: string;
};
