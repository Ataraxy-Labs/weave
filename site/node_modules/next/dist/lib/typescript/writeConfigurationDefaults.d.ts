export declare function getRequiredConfiguration(typescript: typeof import('typescript')): Partial<import('typescript').CompilerOptions>;
export declare function writeConfigurationDefaults(typescriptVersion: string, tsConfigPath: string, isFirstTimeSetup: boolean, hasAppDir: boolean, distDir: string, hasPagesDir: boolean, isolatedDevBuild: boolean | undefined): Promise<void>;
