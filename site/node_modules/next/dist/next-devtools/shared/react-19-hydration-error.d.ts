export declare const REACT_HYDRATION_ERROR_LINK = "https://react.dev/link/hydration-mismatch";
export declare const NEXTJS_HYDRATION_ERROR_LINK = "https://nextjs.org/docs/messages/react-hydration-error";
export declare function isHydrationError(error: Error): boolean;
export declare function isErrorMessageWithComponentStackDiff(msg: string): boolean;
export declare function getHydrationErrorStackInfo(error: Error): {
    message: string | null;
    notes: string | null;
    diff: string | null;
};
