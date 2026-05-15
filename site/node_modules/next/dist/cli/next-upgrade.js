"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "spawnNextUpgrade", {
    enumerable: true,
    get: function() {
        return spawnNextUpgrade;
    }
});
const _child_process = require("child_process");
const _getprojectdir = require("../lib/get-project-dir");
const _getnpxcommand = require("../lib/helpers/get-npx-command");
function spawnNextUpgrade(directory, options) {
    const baseDir = (0, _getprojectdir.getProjectDir)(directory);
    const [upgradeProcessCommand, ...upgradeProcessDefaultArgs] = (0, _getnpxcommand.getNpxCommand)(baseDir).split(' ');
    const upgradeProcessCommandArgs = [
        ...upgradeProcessDefaultArgs,
        // Needs to be bleeding edge (canary) to pick up latest codemods.
        '@next/codemod@canary',
        'upgrade',
        options.revision
    ];
    if (options.verbose) {
        upgradeProcessCommandArgs.push('--verbose');
    }
    const upgradeProcess = (0, _child_process.spawn)(upgradeProcessCommand, upgradeProcessCommandArgs, {
        stdio: 'inherit',
        cwd: baseDir
    });
    upgradeProcess.on('close', (code)=>{
        process.exitCode = code ?? 0;
    });
}

//# sourceMappingURL=next-upgrade.js.map