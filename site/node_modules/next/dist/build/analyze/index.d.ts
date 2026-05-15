export type AnalyzeOptions = {
    dir: string;
    reactProductionProfiling?: boolean;
    noMangling?: boolean;
    appDirOnly?: boolean;
    output?: boolean;
    port?: number;
};
export default function analyze({ dir, reactProductionProfiling, noMangling, appDirOnly, output, port, }: AnalyzeOptions): Promise<void>;
