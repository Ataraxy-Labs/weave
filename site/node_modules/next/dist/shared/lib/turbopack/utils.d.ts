import type { Issue, StyledString, TurbopackResult } from '../../../build/swc/types';
import type { EntryKey } from './entry-key';
import type { NextConfigComplete } from '../../../server/config-shared';
type IssueKey = `${Issue['severity']}-${Issue['filePath']}-${string}-${string}`;
export type IssuesMap = Map<IssueKey, Issue>;
export type EntryIssuesMap = Map<EntryKey, IssuesMap>;
export type TopLevelIssuesMap = IssuesMap;
/**
 * An error generated from emitted Turbopack issues. This can include build
 * errors caused by issues with user code.
 */
export declare class ModuleBuildError extends Error {
    name: string;
}
/**
 * Thin stopgap workaround layer to mimic existing wellknown-errors-plugin in webpack's build
 * to emit certain type of errors into cli.
 */
export declare function isWellKnownError(issue: Issue): boolean;
export declare function getIssueKey(issue: Issue): IssueKey;
export declare function processIssues(currentEntryIssues: EntryIssuesMap, key: EntryKey, result: TurbopackResult, throwIssue: boolean, logErrors: boolean): void;
export declare function formatIssue(issue: Issue): string;
export declare function isRelevantWarning(issue: Issue): boolean;
export declare function renderStyledStringToErrorAnsi(string: StyledString): string;
export declare function isFileSystemCacheEnabledForDev(config: NextConfigComplete): boolean;
export declare function isFileSystemCacheEnabledForBuild(config: NextConfigComplete): boolean;
export {};
