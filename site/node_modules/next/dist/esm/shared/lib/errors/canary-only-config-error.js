export function isStableBuild() {
    return !"16.1.6"?.includes('canary') && !process.env.__NEXT_TEST_MODE && !process.env.NEXT_PRIVATE_LOCAL_DEV;
}
export class CanaryOnlyConfigError extends Error {
    constructor(arg){
        if (typeof arg === 'object' && 'feature' in arg) {
            super(`The experimental feature "${arg.feature}" can only be enabled when using the latest canary version of Next.js.`);
        } else {
            super(arg);
        }
        // This error is meant to interrupt the server start/build process
        // but the stack trace isn't meaningful, as it points to internal code.
        this.stack = undefined;
    }
}

//# sourceMappingURL=canary-only-config-error.js.map