#!/usr/bin/env node
"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "nextAnalyze", {
    enumerable: true,
    get: function() {
        return nextAnalyze;
    }
});
require("../server/lib/cpu-profile");
const _fs = require("fs");
const _picocolors = require("../lib/picocolors");
const _analyze = /*#__PURE__*/ _interop_require_default(require("../build/analyze"));
const _log = require("../build/output/log");
const _utils = require("../server/lib/utils");
const _getprojectdir = require("../lib/get-project-dir");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const nextAnalyze = async (options, directory)=>{
    process.on('SIGTERM', ()=>process.exit(143));
    process.on('SIGINT', ()=>process.exit(130));
    const { profile, mangling, experimentalAppOnly, output, port } = options;
    if (!mangling) {
        (0, _log.warn)(`Mangling is disabled. ${(0, _picocolors.italic)('Note: This may affect performance and should only be used for debugging purposes.')}`);
    }
    if (profile) {
        (0, _log.warn)(`Profiling is enabled. ${(0, _picocolors.italic)('Note: This may affect performance.')}`);
    }
    const dir = (0, _getprojectdir.getProjectDir)(directory);
    if (!(0, _fs.existsSync)(dir)) {
        (0, _utils.printAndExit)(`> No such directory exists as the project root: ${dir}`);
    }
    return (0, _analyze.default)({
        dir,
        reactProductionProfiling: profile,
        noMangling: !mangling,
        appDirOnly: experimentalAppOnly,
        output,
        port
    });
};

//# sourceMappingURL=next-analyze.js.map