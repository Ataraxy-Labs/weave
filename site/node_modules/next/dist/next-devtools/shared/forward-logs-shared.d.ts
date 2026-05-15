export type LogMethod = 'log' | 'info' | 'debug' | 'table' | 'error' | 'assert' | 'dir' | 'dirxml' | 'group' | 'groupCollapsed' | 'groupEnd' | 'trace' | 'warn';
export type ConsoleEntry<T> = {
    kind: 'console';
    method: LogMethod;
    consoleMethodStack: string | null;
    args: Array<{
        kind: 'arg';
        data: T;
    } | {
        kind: 'formatted-error-arg';
        prefix: string;
        stack: string;
    }>;
};
export type ConsoleErrorEntry<T> = {
    kind: 'any-logged-error';
    method: 'error';
    consoleErrorStack: string;
    args: Array<{
        kind: 'arg';
        data: T;
        isRejectionMessage?: boolean;
    } | {
        kind: 'formatted-error-arg';
        prefix: string;
        stack: string | null;
    }>;
};
export type FormattedErrorEntry = {
    kind: 'formatted-error';
    prefix: string;
    stack: string;
    method: 'error';
};
export type ClientLogEntry = ConsoleEntry<unknown> | ConsoleErrorEntry<unknown> | FormattedErrorEntry;
export type ServerLogEntry = ConsoleEntry<string> | ConsoleErrorEntry<string> | FormattedErrorEntry;
export declare const UNDEFINED_MARKER = "__next_tagged_undefined";
export declare function patchConsoleMethod<T extends keyof Console>(methodName: T, wrapper: (methodName: T, ...args: Console[T] extends (...args: infer P) => any ? P : never[]) => void): () => void;
