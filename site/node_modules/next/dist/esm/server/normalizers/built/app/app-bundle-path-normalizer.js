import { Normalizers } from '../../normalizers';
import { PrefixingNormalizer } from '../../prefixing-normalizer';
import { normalizePagePath } from '../../../../shared/lib/page-path/normalize-page-path';
import { UnderscoreNormalizer } from '../../underscore-normalizer';
export class AppBundlePathNormalizer extends PrefixingNormalizer {
    constructor(){
        super('app');
    }
    normalize(page) {
        return super.normalize(normalizePagePath(page));
    }
}
export class DevAppBundlePathNormalizer extends Normalizers {
    constructor(pageNormalizer, isTurbopack){
        const normalizers = [
            // This should normalize the filename to a page.
            pageNormalizer,
            // Normalize the app page to a pathname.
            new AppBundlePathNormalizer()
        ];
        // %5F to _ replacement should only happen with Turbopack.
        if (isTurbopack) {
            normalizers.unshift(new UnderscoreNormalizer());
        }
        super(normalizers);
    }
    normalize(filename) {
        return super.normalize(filename);
    }
}

//# sourceMappingURL=app-bundle-path-normalizer.js.map