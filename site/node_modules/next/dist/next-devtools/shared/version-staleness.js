"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "getStaleness", {
    enumerable: true,
    get: function() {
        return getStaleness;
    }
});
function getStaleness({ installed, staleness, expected }) {
    let text = '';
    let title = '';
    let indicatorClass = '';
    const versionLabel = `Next.js ${installed}`;
    switch(staleness){
        case 'newer-than-npm':
        case 'fresh':
            text = versionLabel;
            title = `Latest available version is detected (${installed}).`;
            indicatorClass = 'fresh';
            break;
        case 'stale-patch':
        case 'stale-minor':
            text = `${versionLabel} (stale)`;
            title = `There is a newer version (${expected}) available, upgrade recommended! `;
            indicatorClass = 'stale';
            break;
        case 'stale-major':
            {
                text = `${versionLabel} (outdated)`;
                title = `An outdated version detected (latest is ${expected}), upgrade is highly recommended!`;
                indicatorClass = 'outdated';
                break;
            }
        case 'stale-prerelease':
            {
                text = `${versionLabel} (stale)`;
                title = `There is a newer canary version (${expected}) available, please upgrade! `;
                indicatorClass = 'stale';
                break;
            }
        case 'unknown':
            text = `${versionLabel} (unknown)`;
            title = 'No Next.js version data was found.';
            indicatorClass = 'unknown';
            break;
        default:
            break;
    }
    return {
        text,
        indicatorClass,
        title
    };
}

if ((typeof exports.default === 'function' || (typeof exports.default === 'object' && exports.default !== null)) && typeof exports.default.__esModule === 'undefined') {
  Object.defineProperty(exports.default, '__esModule', { value: true });
  Object.assign(exports.default, exports);
  module.exports = exports.default;
}

//# sourceMappingURL=version-staleness.js.map