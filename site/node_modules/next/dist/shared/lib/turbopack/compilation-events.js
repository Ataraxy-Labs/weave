"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "backgroundLogCompilationEvents", {
    enumerable: true,
    get: function() {
        return backgroundLogCompilationEvents;
    }
});
const _interop_require_wildcard = require("@swc/helpers/_/_interop_require_wildcard");
const _log = /*#__PURE__*/ _interop_require_wildcard._(require("../../../build/output/log"));
function backgroundLogCompilationEvents(project, { eventTypes, signal } = {}) {
    ;
    (async function() {
        for await (const event of project.compilationEventsSubscribe(eventTypes)){
            if (signal?.aborted) {
                return;
            }
            switch(event.severity){
                case 'EVENT':
                    _log.event(event.message);
                    break;
                case 'TRACE':
                    _log.trace(event.message);
                    break;
                case 'INFO':
                    _log.info(event.message);
                    break;
                case 'WARNING':
                    _log.warn(event.message);
                    break;
                case 'ERROR':
                    _log.error(event.message);
                    break;
                case 'FATAL':
                    _log.error(event.message);
                    break;
                default:
                    break;
            }
        }
    })();
}

//# sourceMappingURL=compilation-events.js.map