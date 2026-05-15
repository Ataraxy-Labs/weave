import type { DraftModeProvider } from '../async-storage/draft-mode-provider';
export declare function draftMode(): Promise<DraftMode>;
declare class DraftMode {
    constructor(provider: null | DraftModeProvider);
    get isEnabled(): boolean;
    enable(): void;
    disable(): void;
}
export {};
