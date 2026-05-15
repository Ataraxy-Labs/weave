import type { AsyncLocalStorage } from 'async_hooks';
import { dynamicAccessAsyncStorageInstance } from './dynamic-access-async-storage-instance';
export interface DynamicAccessAsyncStore {
    readonly abortController: AbortController;
}
export type DynamicAccessStorage = AsyncLocalStorage<DynamicAccessAsyncStore>;
export { dynamicAccessAsyncStorageInstance as dynamicAccessAsyncStorage };
