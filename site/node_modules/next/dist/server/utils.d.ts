export declare function isBlockedPage(page: string): boolean;
type AnyFunc<T> = (this: T, ...args: any) => any;
export declare function debounce<T, F extends AnyFunc<T>>(fn: F, ms: number, maxWait?: number): (this: T, ...passedArgs: Parameters<F>) => void;
export {};
