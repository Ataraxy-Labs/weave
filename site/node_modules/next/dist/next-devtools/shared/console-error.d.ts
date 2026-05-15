declare const digestSym: unique symbol;
type ConsoleError = Error & {
    [digestSym]: 'NEXT_CONSOLE_ERROR';
    environmentName: string;
};
export declare function createConsoleError(message: string | Error, environmentName?: string | null): ConsoleError;
export declare const isConsoleError: (error: any) => error is ConsoleError;
export {};
