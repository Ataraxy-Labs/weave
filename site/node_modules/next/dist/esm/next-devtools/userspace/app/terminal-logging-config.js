export function getTerminalLoggingConfig() {
    try {
        return JSON.parse(process.env.__NEXT_BROWSER_DEBUG_INFO_IN_TERMINAL || 'false');
    } catch  {
        return false;
    }
}
export function getIsTerminalLoggingEnabled() {
    const config = getTerminalLoggingConfig();
    return Boolean(config);
}

//# sourceMappingURL=terminal-logging-config.js.map