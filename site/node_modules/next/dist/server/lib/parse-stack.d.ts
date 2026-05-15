export interface StackFrame {
    file: string | null;
    methodName: string;
    arguments: string[];
    /** 1-based */
    line1: number | null;
    /** 1-based */
    column1: number | null;
}
export declare function parseStack(stack: string, distDir?: string | undefined): StackFrame[];
