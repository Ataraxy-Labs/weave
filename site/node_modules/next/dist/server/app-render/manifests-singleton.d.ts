import type { ActionManifest } from '../../build/webpack/plugins/flight-client-entry-plugin';
import type { ClientReferenceManifest } from '../../build/webpack/plugins/flight-manifest-plugin';
import type { DeepReadonly } from '../../shared/lib/deep-readonly';
export interface ServerModuleMap {
    readonly [name: string]: {
        readonly id: string | number;
        readonly name: string;
        readonly chunks: Readonly<Array<string>>;
        readonly async?: boolean;
    };
}
/**
 * Checks if the requested action has a worker for the current page.
 * If not, it returns the first worker that has a handler for the action.
 */
export declare function selectWorkerForForwarding(actionId: string, pageName: string): string | undefined;
export declare function setManifestsSingleton({ page, clientReferenceManifest, serverActionsManifest, }: {
    page: string;
    clientReferenceManifest: DeepReadonly<ClientReferenceManifest>;
    serverActionsManifest: DeepReadonly<ActionManifest>;
}): void;
export declare function getClientReferenceManifest(): DeepReadonly<ClientReferenceManifest>;
export declare function getServerActionsManifest(): DeepReadonly<ActionManifest>;
export declare function getServerModuleMap(): ServerModuleMap;
