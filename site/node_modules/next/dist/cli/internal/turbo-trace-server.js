"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "startTurboTraceServerCli", {
    enumerable: true,
    get: function() {
        return startTurboTraceServerCli;
    }
});
const _swc = require("../../build/swc");
async function startTurboTraceServerCli(file, port) {
    let bindings = await (0, _swc.loadBindings)();
    bindings.turbo.startTurbopackTraceServer(file, port);
}

//# sourceMappingURL=turbo-trace-server.js.map