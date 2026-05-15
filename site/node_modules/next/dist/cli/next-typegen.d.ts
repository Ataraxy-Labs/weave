#!/usr/bin/env node
export type NextTypegenOptions = {
    dir?: string;
};
declare const nextTypegen: (_options: NextTypegenOptions, directory?: string) => Promise<void>;
export { nextTypegen };
