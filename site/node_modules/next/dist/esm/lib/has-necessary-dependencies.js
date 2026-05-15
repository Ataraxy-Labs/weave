import { existsSync, realpathSync } from 'fs';
import { resolveFrom } from './resolve-from';
import { dirname, join, relative } from 'path';
export function hasNecessaryDependencies(baseDir, requiredPackages) {
    let resolutions = new Map();
    const missingPackages = [];
    for (const p of requiredPackages){
        try {
            const pkgPath = realpathSync(resolveFrom(baseDir, `${p.pkg}/package.json`));
            const pkgDir = dirname(pkgPath);
            resolutions.set(join(p.pkg, 'package.json'), pkgPath);
            if (p.exportsRestrict) {
                const fileNameToVerify = relative(p.pkg, p.file);
                if (fileNameToVerify) {
                    const fileToVerify = join(pkgDir, fileNameToVerify);
                    if (existsSync(fileToVerify)) {
                        resolutions.set(p.pkg, fileToVerify);
                    } else {
                        missingPackages.push(p);
                        continue;
                    }
                } else {
                    resolutions.set(p.pkg, pkgPath);
                }
            } else {
                resolutions.set(p.pkg, resolveFrom(baseDir, p.file));
            }
        } catch (_) {
            missingPackages.push(p);
            continue;
        }
    }
    return {
        resolved: resolutions,
        missing: missingPackages
    };
}

//# sourceMappingURL=has-necessary-dependencies.js.map