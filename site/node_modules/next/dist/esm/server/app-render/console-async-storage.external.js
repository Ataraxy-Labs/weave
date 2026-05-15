// Share the instance module in the next-shared layer
import { consoleAsyncStorageInstance } from './console-async-storage-instance' with {
    'turbopack-transition': 'next-shared'
};
export { consoleAsyncStorageInstance as consoleAsyncStorage };

//# sourceMappingURL=console-async-storage.external.js.map