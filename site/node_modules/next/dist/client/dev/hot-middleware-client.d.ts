declare const _default: () => {
    subscribeToHmrEvent(handler: (message: import("../../server/dev/hot-reloader-types").AddedPageMessage | import("../../server/dev/hot-reloader-types").RemovedPageMessage | import("../../server/dev/hot-reloader-types").ReloadPageMessage | import("../../server/dev/hot-reloader-types").DevPagesManifestUpdateMessage) => void): void;
    onUnrecoverableError(): void;
    addTurbopackMessageListener(cb: (msg: import("../../server/dev/hot-reloader-types").TurbopackMessageSentToBrowser) => void): void;
    sendTurbopackMessage(msg: string): void;
    handleUpdateError(err: unknown): void;
};
export default _default;
