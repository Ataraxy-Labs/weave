let ClientReact = null;
export function registerClientReact(react) {
    ClientReact = react;
}
export function getClientReact() {
    return ClientReact;
}
let ServerReact = null;
export function registerServerReact(react) {
    ServerReact = react;
}
export function getServerReact() {
    return ServerReact;
}

//# sourceMappingURL=runtime-reacts.external.js.map