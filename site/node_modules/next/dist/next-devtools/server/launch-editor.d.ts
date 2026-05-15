export declare function escapeApplescriptStringFragment(input: string): string;
export declare function launchEditor(fileName: string, line1: number, column1: number): void;
/**
 * Open the file in editor if exists, otherwise return an error
 * @param file `file:` URL, or absolute path or relative path. Relative paths must
 *             relative to `nextRootDirectory`.
 */
export declare function openFileInEditor(file: string, line1: number, column1: number, nextRootDirectory: string): Promise<{
    found: boolean;
    error: unknown | null;
}>;
