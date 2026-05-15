interface NextUpgradeOptions {
    revision: string;
    verbose: boolean;
}
export declare function spawnNextUpgrade(directory: string | undefined, options: NextUpgradeOptions): void;
export {};
