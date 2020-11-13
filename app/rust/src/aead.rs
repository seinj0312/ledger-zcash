use aes::block_cipher_trait::generic_array::GenericArray;
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use chacha20poly1305::aead::{AeadInPlace, NewAead};
use chacha20poly1305::aead::heapless::{consts::U128, Vec};
use typenum::UInt;

use crate::constants::{
    COMPACT_NOTE_SIZE, ENC_CIPHERTEXT_SIZE, ENC_COMPACT_SIZE, NOTE_PLAINTEXT_SIZE,
    OUT_PLAINTEXT_SIZE,
};

const NONCE: [u8; 12] = [0u8; 12]; // FIXME: 128-bits; unique per message

pub fn aead_encryptnote(
    key: &GenericArray<u8, typenum::U32>,
    plaintext: [u8; NOTE_PLAINTEXT_SIZE],
) -> Vec<u8, typenum::U580> {
    let key_array = Key::from_slice(&key);
    let nonce = Nonce::from_slice(&NONCE);

    let mut buffer: Vec<u8, typenum::U580> = Vec::new();
    buffer
        .extend_from_slice(&plaintext)
        .expect("could not extend");

    ChaCha20Poly1305::new(&key_array)
        .encrypt_in_place(nonce, &[], &mut buffer)
        .expect("encryption failure!");

    buffer
}

pub fn aead_encrypt_outciphertext(
    key: &GenericArray<u8, typenum::U32>,
    plaintext: [u8; OUT_PLAINTEXT_SIZE],
) -> Vec<u8, typenum::U80> {
    let key_array = Key::from_slice(&key);
    let nonce = Nonce::from_slice(&NONCE);

    let mut buffer: Vec<u8, typenum::U80> = Vec::new();
    buffer
        .extend_from_slice(&plaintext)
        .expect("could not extend");

    ChaCha20Poly1305::new(&key_array)
        .encrypt_in_place(nonce, &[], &mut buffer)
        .expect("encryption failure!");

    buffer
}

pub fn aead_decryptnote(
    key: &GenericArray<u8, typenum::U32>,
    ciphertext: [u8; ENC_CIPHERTEXT_SIZE],
) -> Vec<u8, typenum::U580> {
    let key_array = Key::from_slice(&key);
    let nonce = Nonce::from_slice(&NONCE);

    let mut buffer: Vec<u8, typenum::U580> = Vec::new();
    buffer
        .extend_from_slice(&ciphertext)
        .expect("could not extend");

    ChaCha20Poly1305::new(&key_array)
        .decrypt_in_place(nonce, &[], &mut buffer)
        .expect("decryption failure!");

    buffer
}

pub fn aead_encryptcompact(
    key: &GenericArray<u8, typenum::U32>,
    plaintext: [u8; COMPACT_NOTE_SIZE],
) -> Vec<u8, typenum::U68> {
    let key_array = Key::from_slice(&key);
    let nonce = Nonce::from_slice(&NONCE);

    let mut buffer: Vec<u8, typenum::U68> = Vec::new();
    buffer
        .extend_from_slice(&plaintext)
        .expect("could not extend");

    ChaCha20Poly1305::new(&key_array)
        .encrypt_in_place(nonce, &[], &mut buffer)
        .expect("encryption failure!");

    buffer
}

pub fn aead_decryptcompact(
    key: &GenericArray<u8, typenum::U32>,
    ciphertext: [u8; ENC_COMPACT_SIZE],
) -> Vec<u8, typenum::U68> {
    let key_array = Key::from_slice(&key);
    let nonce = Nonce::from_slice(&NONCE);

    let mut buffer: Vec<u8, typenum::U68> = Vec::new();
    buffer
        .extend_from_slice(&ciphertext)
        .expect("could not extend");

    ChaCha20Poly1305::new(&key_array)
        .decrypt_in_place(nonce, &[], &mut buffer)
        .expect("decryption failure!");

    buffer
}

#[cfg(test)]
mod tests {
    use aes::block_cipher_trait::generic_array::GenericArray;

    use crate::aead::{aead_decryptnote, aead_encryptnote};

    const KEY_ENC: [u8; 32] = [
        0x6d, 0xf8, 0x5b, 0x17, 0x89, 0xb0, 0xb7, 0x8b, 0x46, 0x10, 0xf2, 0x5d, 0x36, 0x8c, 0xb5,
        0x11, 0x14, 0x0a, 0x7c, 0x0a, 0xf3, 0xbc, 0x3d, 0x2a, 0x22, 0x6f, 0x92, 0x7d, 0xe6, 0x02,
        0xa7, 0xf1,
    ];

    const P_ENC: [u8; 564] = [
        0x01, 0xdc, 0xe7, 0x7e, 0xbc, 0xec, 0x0a, 0x26, 0xaf, 0xd6, 0x99, 0x8c, 0x00, 0xe1, 0xf5,
        0x05, 0x00, 0x00, 0x00, 0x00, 0x39, 0x17, 0x6d, 0xac, 0x39, 0xac, 0xe4, 0x98, 0x0e, 0xcc,
        0x8d, 0x77, 0x8e, 0x89, 0x86, 0x02, 0x55, 0xec, 0x36, 0x15, 0x06, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf6, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    const C_ENC: [u8; 580] = [
        0xbd, 0xcb, 0x94, 0x72, 0xa1, 0xac, 0xad, 0xf1, 0xd0, 0x82, 0x07, 0xf6, 0x3c, 0xaf, 0x4f,
        0x3a, 0x76, 0x3c, 0x67, 0xd0, 0x66, 0x56, 0x0a, 0xd9, 0x6c, 0x1e, 0xf9, 0x52, 0xf8, 0x46,
        0xa9, 0xc2, 0x80, 0x82, 0xdd, 0xef, 0x45, 0x21, 0xf6, 0x82, 0x54, 0x76, 0xad, 0xe3, 0x2e,
        0xeb, 0x34, 0x64, 0x06, 0xa5, 0xee, 0xc9, 0x4b, 0x4a, 0xb9, 0xe4, 0x55, 0x12, 0x42, 0xb1,
        0x44, 0xa4, 0xf8, 0xc8, 0x28, 0xbc, 0x19, 0x7f, 0x3e, 0x92, 0x5f, 0x61, 0x7f, 0xc4, 0xb9,
        0xc1, 0xb1, 0x53, 0xad, 0x15, 0x3a, 0x3c, 0x56, 0xf8, 0x1f, 0xc4, 0x8b, 0xf5, 0x4e, 0x6e,
        0xe8, 0x89, 0x5f, 0x27, 0x8c, 0x5e, 0x4c, 0x6a, 0xe7, 0xa8, 0xa0, 0x23, 0x86, 0x70, 0x85,
        0xb4, 0x07, 0xbe, 0xce, 0x40, 0x0b, 0xc6, 0xaa, 0xec, 0x06, 0xaf, 0xf8, 0xb0, 0x49, 0xbc,
        0xb2, 0x63, 0x63, 0xc6, 0xde, 0x01, 0x8d, 0x2d, 0xa0, 0x41, 0xcc, 0x2e, 0xb8, 0xd0, 0x86,
        0x4a, 0x70, 0xdf, 0x68, 0x47, 0xb3, 0x37, 0x5a, 0x31, 0x86, 0x6c, 0x49, 0xa8, 0x02, 0x5a,
        0xd7, 0x17, 0xe7, 0x79, 0xbd, 0x0f, 0xb5, 0xce, 0xed, 0x3e, 0xc4, 0x40, 0x8e, 0x18, 0x50,
        0x69, 0x4b, 0xa3, 0x56, 0x39, 0xdd, 0x8b, 0x55, 0xd2, 0xbf, 0xdf, 0xc6, 0x40, 0x6c, 0x78,
        0xc0, 0x0e, 0xb5, 0xfc, 0x48, 0x76, 0x4b, 0xf4, 0xd8, 0x4d, 0xe1, 0xa0, 0x26, 0xd9, 0x02,
        0x86, 0x60, 0xa9, 0xa5, 0xc1, 0xc5, 0x94, 0xb8, 0x15, 0x8c, 0x69, 0x1e, 0x50, 0x68, 0xc8,
        0x51, 0xda, 0xfa, 0x30, 0x10, 0xe3, 0x9b, 0x70, 0xc4, 0x66, 0x83, 0x73, 0xbb, 0x59, 0xac,
        0x53, 0x07, 0x0c, 0x7b, 0x3f, 0x76, 0x62, 0x03, 0x84, 0x27, 0xb3, 0x72, 0xfd, 0x75, 0x36,
        0xe5, 0x4d, 0x8c, 0x8e, 0x61, 0x56, 0x2c, 0xb0, 0xe5, 0x7e, 0xf7, 0xb4, 0x43, 0xde, 0x5e,
        0x47, 0x8f, 0x4b, 0x02, 0x9c, 0x36, 0xaf, 0x71, 0x27, 0x1a, 0x0f, 0x9d, 0x57, 0xbe, 0x80,
        0x1b, 0xc4, 0xf2, 0x61, 0x8d, 0xc4, 0xf0, 0xab, 0xd1, 0x5f, 0x0b, 0x42, 0x0c, 0x11, 0x14,
        0xbb, 0xd7, 0x27, 0xe4, 0xb3, 0x1a, 0x6a, 0xaa, 0xd8, 0xfe, 0x53, 0xb7, 0xdf, 0x60, 0xb4,
        0xe0, 0xc9, 0xe9, 0x45, 0x7b, 0x89, 0x3f, 0x20, 0xec, 0x18, 0x61, 0x1e, 0x68, 0x03, 0x05,
        0xfe, 0x04, 0xba, 0x3b, 0x8d, 0x30, 0x1f, 0x5c, 0xd8, 0x2c, 0x2c, 0x8d, 0x1c, 0x58, 0x5d,
        0x51, 0x15, 0x4b, 0x46, 0x88, 0xff, 0x5a, 0x35, 0x0b, 0x60, 0xae, 0x30, 0xda, 0x4f, 0x74,
        0xc3, 0xd5, 0x5c, 0x73, 0xda, 0xe8, 0xad, 0x9a, 0xb8, 0x0b, 0xbb, 0x5d, 0xdf, 0x1b, 0xea,
        0xec, 0x12, 0x0f, 0xc4, 0xf7, 0x8d, 0xe5, 0x4f, 0xef, 0xe1, 0xa8, 0x41, 0x35, 0x79, 0xfd,
        0xce, 0xa2, 0xf6, 0x56, 0x74, 0x10, 0x4c, 0xba, 0xac, 0x7e, 0x0d, 0xe5, 0x08, 0x3d, 0xa7,
        0xb1, 0xb7, 0xf2, 0xe9, 0x43, 0x70, 0xdd, 0x0a, 0x3e, 0xed, 0x71, 0x50, 0x36, 0x54, 0x2f,
        0xa4, 0x0e, 0xd4, 0x89, 0x2b, 0xaa, 0xfb, 0x57, 0x2e, 0xe0, 0xf9, 0x45, 0x9c, 0x1c, 0xbe,
        0x3a, 0xd1, 0xb6, 0xaa, 0xf1, 0x1f, 0x54, 0x93, 0x59, 0x52, 0xbe, 0x6b, 0x95, 0x38, 0xa9,
        0xa3, 0x9e, 0xde, 0x64, 0x2b, 0xb0, 0xcd, 0xac, 0x1c, 0x09, 0x09, 0x2c, 0xd7, 0x11, 0x16,
        0x0a, 0x8d, 0x45, 0x19, 0xb4, 0xce, 0x20, 0xff, 0xf6, 0x61, 0x2b, 0xc7, 0xb0, 0x53, 0x93,
        0xbb, 0x7e, 0x96, 0xf8, 0xea, 0x4b, 0xbc, 0x97, 0x83, 0x1f, 0x20, 0x46, 0xe1, 0xcb, 0x5a,
        0x2c, 0xe7, 0xca, 0x36, 0xfd, 0x06, 0xab, 0x39, 0x56, 0xa8, 0x03, 0xd4, 0x32, 0x5a, 0xae,
        0x72, 0xef, 0xb7, 0x07, 0xca, 0xa0, 0x44, 0xd3, 0xf8, 0xfc, 0x7d, 0x09, 0x46, 0xbe, 0xb1,
        0x1c, 0xdd, 0xc8, 0x53, 0xdb, 0xcf, 0x24, 0x3a, 0xf3, 0xe5, 0x92, 0xb8, 0x1d, 0xb3, 0x64,
        0x19, 0xd3, 0x4a, 0x4b, 0xb1, 0xee, 0x53, 0xc1, 0xa1, 0xba, 0x51, 0xc1, 0x8b, 0x2e, 0xe9,
        0x2d, 0xb4, 0xbf, 0x5f, 0xce, 0xeb, 0x82, 0x0e, 0x8c, 0x58, 0xf8, 0x16, 0x6c, 0x3a, 0xcb,
        0xf7, 0x61, 0xb5, 0xb1, 0xf2, 0x9c, 0x3f, 0x11, 0x81, 0x67, 0xbb, 0x6c, 0xdb, 0x23, 0x30,
        0x35, 0x29, 0x6a, 0xd4, 0x0e, 0x8a, 0xa0, 0xce, 0xf5, 0x70,
    ];

    #[test]
    fn test_encrypt() {
        let key = GenericArray::from_slice(&KEY_ENC);
        let cyphertext = aead_encryptnote(key, P_ENC);

        assert_eq!(cyphertext[..], C_ENC[..]);
    }

    #[test]
    fn test_decrypt() {
        let key = GenericArray::from_slice(&KEY_ENC);
        let plaintext = aead_decryptnote(&key, C_ENC);

        assert_eq!(plaintext[..], P_ENC[..]);
    }
}
