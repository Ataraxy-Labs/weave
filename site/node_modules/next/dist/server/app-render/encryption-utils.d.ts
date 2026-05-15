export declare function arrayBufferToString(buffer: ArrayBuffer | Uint8Array<ArrayBufferLike>): string;
export declare function stringToUint8Array(binary: string): Uint8Array<ArrayBuffer>;
export declare function encrypt(key: CryptoKey, iv: Uint8Array<ArrayBuffer>, data: Uint8Array<ArrayBuffer>): Promise<ArrayBuffer>;
export declare function decrypt(key: CryptoKey, iv: Uint8Array<ArrayBuffer>, data: Uint8Array<ArrayBuffer>): Promise<ArrayBuffer>;
export declare function getActionEncryptionKey(): Promise<CryptoKey>;
