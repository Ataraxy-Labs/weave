"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "generatePreviewKeys", {
    enumerable: true,
    get: function() {
        return generatePreviewKeys;
    }
});
const _path = /*#__PURE__*/ _interop_require_default(require("path"));
const _fs = /*#__PURE__*/ _interop_require_default(require("fs"));
const _crypto = /*#__PURE__*/ _interop_require_default(require("crypto"));
const _cachedir = require("../server/cache-dir");
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : {
        default: obj
    };
}
const CONFIG_FILE = '.previewinfo';
const PREVIEW_ID = 'previewModeId';
const PREVIEW_SIGNING_KEY = 'previewModeSigningKey';
const PREVIEW_ENCRYPTION_KEY = 'previewModeEncryptionKey';
const PREVIEW_EXPIRE_AT = 'expireAt';
const EXPIRATION = 1000 * 60 * 60 * 24 * 14 // 14 days
;
async function writeCache(distDir, config) {
    const cacheBaseDir = (0, _cachedir.getStorageDirectory)(distDir);
    if (!cacheBaseDir) return;
    const configPath = _path.default.join(cacheBaseDir, CONFIG_FILE);
    if (!_fs.default.existsSync(cacheBaseDir)) {
        await _fs.default.promises.mkdir(cacheBaseDir, {
            recursive: true
        });
    }
    await _fs.default.promises.writeFile(configPath, JSON.stringify({
        [PREVIEW_ID]: config.previewModeId,
        [PREVIEW_SIGNING_KEY]: config.previewModeSigningKey,
        [PREVIEW_ENCRYPTION_KEY]: config.previewModeEncryptionKey,
        [PREVIEW_EXPIRE_AT]: Date.now() + EXPIRATION
    }));
}
function generateConfig() {
    return {
        previewModeId: _crypto.default.randomBytes(16).toString('hex'),
        previewModeSigningKey: _crypto.default.randomBytes(32).toString('hex'),
        previewModeEncryptionKey: _crypto.default.randomBytes(32).toString('hex')
    };
}
async function generatePreviewKeys({ distDir, isBuild }) {
    const cacheBaseDir = (0, _cachedir.getStorageDirectory)(distDir);
    if (!cacheBaseDir) {
        // There's no persistent storage available. We generate a new config.
        // This also covers development time.
        return generateConfig();
    }
    const configPath = _path.default.join(cacheBaseDir, CONFIG_FILE);
    async function tryReadCachedConfig() {
        if (!_fs.default.existsSync(configPath)) return false;
        try {
            const config = JSON.parse(await _fs.default.promises.readFile(configPath, 'utf8'));
            if (!config) return false;
            if (typeof config[PREVIEW_ID] !== 'string' || typeof config[PREVIEW_ENCRYPTION_KEY] !== 'string' || typeof config[PREVIEW_SIGNING_KEY] !== 'string' || typeof config[PREVIEW_EXPIRE_AT] !== 'number') {
                return false;
            }
            // For build time, we need to rotate the key if it's expired. Otherwise
            // (next start) we have to keep the key as it is so the runtime key matches
            // the build time key.
            if (isBuild && config[PREVIEW_EXPIRE_AT] < Date.now()) {
                return false;
            }
            return {
                previewModeId: config[PREVIEW_ID],
                previewModeSigningKey: config[PREVIEW_SIGNING_KEY],
                previewModeEncryptionKey: config[PREVIEW_ENCRYPTION_KEY]
            };
        } catch (e) {
            // Broken config file. We should generate a new key and overwrite it.
            return false;
        }
    }
    const maybeValidConfig = await tryReadCachedConfig();
    if (maybeValidConfig !== false) {
        return maybeValidConfig;
    }
    const config = generateConfig();
    await writeCache(distDir, config);
    return config;
}

//# sourceMappingURL=preview-key-utils.js.map