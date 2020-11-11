use aes::block_cipher_trait::generic_array::{GenericArray, GenericArrayImplEven};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use chacha20poly1305::aead::heapless::{consts::U32, consts::*, Vec};

use crate::aead::*;
use crate::bolos::{blake2b32_with_personalization, c_zemu_log_stack};
use crate::commitments::{bytes_to_extended, bytes_to_u64, note_commitment, write_u64_tobytes};
use crate::constants::{
    COMPACT_NOTE_SIZE, ENC_CIPHERTEXT_SIZE, ENC_COMPACT_SIZE, NOTE_PLAINTEXT_SIZE,
    OUT_CIPHERTEXT_SIZE, OUT_PLAINTEXT_SIZE,
};
use crate::pedersen::extended_to_u_bytes;
use crate::zeccrypto::*;
use crate::zip32::{default_pkd, group_hash_from_div, multwithgd, pkd_group_hash};

pub fn parse_note_metadata(ivk: &[u8; 32], cmu: &[u8; 32], plaintext: &[u8]) -> bool {
    match plaintext[0] {
        0x01 => (),
        _ => return false,
    }

    let mut d = [0u8; 11];
    d.copy_from_slice(&plaintext[1..12]);

    let pk_d = default_pkd(ivk, &d);

    let mut rcm = [0u8; 32];
    rcm.copy_from_slice(&plaintext[20..COMPACT_NOTE_SIZE]);

    let mut value = [0u8; 8];
    value.copy_from_slice(&plaintext[12..20]);

    let newvalue = LittleEndian::read_u64(&value);

    let g_d = pkd_group_hash(&d);

    let commit = note_commitment(newvalue, &g_d, &pk_d, &rcm);
    let newcmu = extended_to_u_bytes(&commit);

    if *cmu != newcmu {
        // write the rcm plus cmu values in flash, this is needed for a new transaction later on
        return false;
    }
    true
}

#[inline(never)]
pub fn try_sapling_note_decryption(
    ivk: &[u8; 32],
    epk: &[u8; 32],
    cmu: &[u8; 32], //where is this?
    enc_ciphertext: &[u8; ENC_CIPHERTEXT_SIZE],
) -> bool {
    let shared_secret = sapling_ka_agree(ivk, epk);
    let key = kdf_sapling(&shared_secret, epk);
    let k = GenericArray::from_slice(&key);

    let plaintext = aead_decryptnote(&k, *enc_ciphertext);

    let mut memo = [0u8; 512];
    memo.copy_from_slice(&plaintext[COMPACT_NOTE_SIZE..NOTE_PLAINTEXT_SIZE]);

    parse_note_metadata(ivk, cmu, &plaintext[0..COMPACT_NOTE_SIZE])
}

#[inline(never)]
pub fn encrypt_compact_plaintext(
    esk: &[u8; 32],  //random, generate self
    epk: &[u8; 32],  //generate from esk and g_d from host (from diversifier)
    d: &[u8; 11],    //from host
    pk_d: &[u8; 32], //from host
    value: u64,      //from host
    rcm: &[u8; 32],  //random, generate self and store to flash
) -> Vec<u8, typenum::U68> {
    let shared_secret = sapling_ka_agree(esk, pk_d);
    let key = kdf_sapling(&shared_secret, epk);
    let k = GenericArray::from_slice(&key);

    let mut input = [0; COMPACT_NOTE_SIZE];
    input[0] = 1;
    input[1..12].copy_from_slice(d);

    let mut vbytes = [0u8; 8];
    LittleEndian::write_u64(&mut vbytes, value);

    input[12..20].copy_from_slice(&vbytes);
    input[20..COMPACT_NOTE_SIZE].copy_from_slice(rcm);

    let output = aead_encryptcompact(&k, input);
    output
}

#[inline(never)]
pub fn try_sapling_compact_decryption(
    ivk: &[u8; 32],                          //this should be computed by self
    epk: &[u8; 32],                          //this is stored in a incoming transaction
    cmu: &[u8; 32],                          //this is stored in a incoming transaction
    enc_ciphertext: &[u8; ENC_COMPACT_SIZE], //this is stored in a incoming transaction
) -> bool {
    let shared_secret = sapling_ka_agree(ivk, epk);
    let key = kdf_sapling(&shared_secret, epk);
    let k = GenericArray::from_slice(&key);

    let plaintext = aead_decryptcompact(&k, *enc_ciphertext);

    parse_note_metadata(ivk, cmu, &plaintext)
}

#[no_mangle]
pub extern "C" fn blake2b_prf(input_ptr: *const [u8; 128], out_ptr: *mut [u8; 32]) {
    c_zemu_log_stack(b"inside_blake2bprfock\x00".as_ref());
    let input = unsafe { &*input_ptr }; //ovk, cv, cmu, epk
    pub const PRF_OCK_PERSONALIZATION: &[u8; 16] = b"Zcash_Derive_ock";
    let hash = blake2b32_with_personalization(PRF_OCK_PERSONALIZATION, input);
    let output = unsafe { &mut *out_ptr }; //ovk, cv, cmu, epk
    output.copy_from_slice(&hash);
}

#[no_mangle]
pub extern "C" fn encrypt_out(
    key_ptr: *const [u8; 32],
    input_ptr: *mut [u8; 64],
    output_ptr: *mut [u8; 80],
) {
    c_zemu_log_stack(b"inside_encryptout\x00".as_ref());

    let input = unsafe { *input_ptr };
    let output = unsafe { &mut *output_ptr };
    let key = unsafe { &*key_ptr };
    let k: &GenericArray<u8, typenum::U32> = GenericArray::from_slice(key);
    let enc: Vec<u8, typenum::U80> = aead_encrypt_outciphertext(k, input);
    output.copy_from_slice(&enc);
}

/// Generates `outCiphertext` for this note.
#[inline(never)]
pub fn encrypt_outgoing_plaintext(
    pk_d: &[u8; 32], //self
    esk: &[u8; 32],  //self
    ovk: &[u8; 32],  //self
    cv: &[u8; 32],   //self
    cmu: &[u8; 32],  //self
    epk: &[u8; 32],  //self
) -> Vec<u8, typenum::U80> {
    let key = prf_ock(ovk, cv, cmu, epk);
    let k = GenericArray::from_slice(&key);

    let mut input = [0u8; OUT_PLAINTEXT_SIZE];
    input[0..32].copy_from_slice(pk_d);
    input[32..64].copy_from_slice(esk);
    aead_encrypt_outciphertext(k, input)
}

#[no_mangle]
pub extern "C" fn get_epk(
    esk_ptr: *const [u8; 32],
    d_ptr: *const [u8; 11],
    output_ptr: *mut [u8; 32],
) {
    c_zemu_log_stack(b"inside_getepk\x00".as_ref());
    let esk = unsafe { &*esk_ptr }; //ovk, cv, cmu, epk
    let d = unsafe { &*d_ptr };
    let output = unsafe { &mut *output_ptr };
    let epk = multwithgd(esk, d);
    output.copy_from_slice(&epk);
}

#[no_mangle]
pub extern "C" fn ka_to_key(
    esk_ptr: *const [u8; 32],
    pkd_ptr: *const [u8; 32],
    epk_ptr: *const [u8; 32],
    output_ptr: *mut [u8; 32],
) {
    c_zemu_log_stack(b"inside_katokey\x00".as_ref());
    let esk = unsafe { &*esk_ptr }; //ovk, cv, cmu, epk
    let pkd = unsafe { &*pkd_ptr };
    let epk = unsafe { &*epk_ptr };
    let shared_secret = sapling_ka_agree(esk, pkd);
    let key = kdf_sapling(&shared_secret, epk);
    let output = unsafe { &mut *output_ptr }; //ovk, cv, cmu, epk
    output.copy_from_slice(&key);
}

#[no_mangle]
pub extern "C" fn prepare_enccompact_input(
    d_ptr: *const [u8; 11],
    value: u64,
    rcm_ptr: *const [u8; 32],
    memotype: u8,
    output_ptr: *mut [u8; COMPACT_NOTE_SIZE + 1],
) {
    c_zemu_log_stack(b"inside enccompactinput\x00".as_ref());
    let d = unsafe { &*d_ptr };
    let rcm = unsafe { &*rcm_ptr };

    let output = unsafe { &mut *output_ptr };

    let mut input = [0; COMPACT_NOTE_SIZE + 1];
    input[0] = 2;
    input[1..12].copy_from_slice(d);

    let mut vbytes = [0u8; 8];
    LittleEndian::write_u64(&mut vbytes, value);

    input[12..20].copy_from_slice(&vbytes);
    input[20..COMPACT_NOTE_SIZE].copy_from_slice(rcm);
    input[COMPACT_NOTE_SIZE] = memotype;
    c_zemu_log_stack(b"before copy\x00".as_ref());
    output.copy_from_slice(&input);
}

#[no_mangle]
pub extern "C" fn encryptcompact(
    key_ptr: *const [u8; 32],
    input_ptr: *const [u8; COMPACT_NOTE_SIZE],
    output_ptr: *mut [u8; COMPACT_NOTE_SIZE],
) {
    c_zemu_log_stack(b"inside encryptcompact\x00".as_ref());

    let key = unsafe { *key_ptr };
    let input = unsafe { *input_ptr };

    let k = GenericArray::from_slice(&key);
    let output = unsafe { &mut *output_ptr };
    let buffer = aead_encryptcompact(k, input);

    output.copy_from_slice(&buffer[0..COMPACT_NOTE_SIZE]);
}

/*
#[no_mangle]
pub extern "C" fn encrypt_enctext(
    key_ptr: *const [u8; 32],
    input_ptr: *mut [u8; 564],
    output_ptr: *mut [u8; 580],
) {
    c_zemu_log_stack(b"inside enccompactinput\x00".as_ref());
    let input = unsafe { &*input_ptr };
    let output = unsafe { &mut *output_ptr };
    let key = unsafe { &*key_ptr };
    let k = GenericArray::from_slice(key);
    output.copy_from_slice(&aead_encryptnote(k, *input));
}
*/

#[inline(never)]
pub fn encrypt_note_plaintext(
    esk: &[u8; 32],   //random, generate self
    epk: &[u8; 32],   //generate from esk and g_d from host (from diversifier)
    d: &[u8; 11],     //from host
    pk_d: &[u8; 32],  //from host
    value: u64,       //from host
    rcm: &[u8; 32],   //generate self
    memo: &[u8; 512], //from host
) -> Vec<u8, typenum::U580> {
    let shared_secret = sapling_ka_agree(esk, pk_d);
    let key = kdf_sapling(&shared_secret, epk);
    let k = GenericArray::from_slice(&key);

    let mut input = [0; NOTE_PLAINTEXT_SIZE];
    input[0] = 1;
    input[1..12].copy_from_slice(d);

    let mut vbytes = [0u8; 8];
    LittleEndian::write_u64(&mut vbytes, value);

    input[12..20].copy_from_slice(&vbytes);
    input[20..COMPACT_NOTE_SIZE].copy_from_slice(rcm);

    input[COMPACT_NOTE_SIZE..NOTE_PLAINTEXT_SIZE].copy_from_slice(memo);

    aead_encryptnote(&k, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_katokey() {
        let esk = [
            0x81, 0xc7, 0xb2, 0x17, 0x1f, 0xf4, 0x41, 0x52, 0x50, 0xca, 0xc0, 0x1f, 0x59, 0x82,
            0xfd, 0x8f, 0x49, 0x61, 0x9d, 0x61, 0xad, 0x78, 0xf6, 0x83, 0x0b, 0x3c, 0x60, 0x61,
            0x45, 0x96, 0x2a, 0x0e,
        ];
        let pk_d = [
            0xdb, 0x4c, 0xd2, 0xb0, 0xaa, 0xc4, 0xf7, 0xeb, 0x8c, 0xa1, 0x31, 0xf1, 0x65, 0x67,
            0xc4, 0x45, 0xa9, 0x55, 0x51, 0x26, 0xd3, 0xc2, 0x9f, 0x14, 0xe3, 0xd7, 0x76, 0xe8,
            0x41, 0xae, 0x74, 0x15,
        ];

        let epk = [
            0xde, 0xd6, 0x8f, 0x05, 0xc6, 0x58, 0xfc, 0xae, 0x5a, 0xe2, 0x18, 0x64, 0x6f, 0xf8,
            0x44, 0x40, 0x6f, 0x84, 0x42, 0x67, 0x84, 0x04, 0x0d, 0x0b, 0xef, 0x2b, 0x09, 0xcb,
            0x38, 0x48, 0xc4, 0xdc,
        ];

        let mut output = [0u8; 32];

        ka_to_key(
            esk.as_ptr() as *const [u8; 32],
            pk_d.as_ptr() as *const [u8; 32],
            epk.as_ptr() as *const [u8; 32],
            output.as_mut_ptr() as *mut [u8; 32],
        );

        let shared_secret = sapling_ka_agree(&esk, &pk_d);
        let key = kdf_sapling(&shared_secret, &epk);

        assert_eq!(output, key);
    }

    #[test]
    fn test_note_encryption() {
        let esk = [
            0x81, 0xc7, 0xb2, 0x17, 0x1f, 0xf4, 0x41, 0x52, 0x50, 0xca, 0xc0, 0x1f, 0x59, 0x82,
            0xfd, 0x8f, 0x49, 0x61, 0x9d, 0x61, 0xad, 0x78, 0xf6, 0x83, 0x0b, 0x3c, 0x60, 0x61,
            0x45, 0x96, 0x2a, 0x0e,
        ];
        let epk = [
            0xde, 0xd6, 0x8f, 0x05, 0xc6, 0x58, 0xfc, 0xae, 0x5a, 0xe2, 0x18, 0x64, 0x6f, 0xf8,
            0x44, 0x40, 0x6f, 0x84, 0x42, 0x67, 0x84, 0x04, 0x0d, 0x0b, 0xef, 0x2b, 0x09, 0xcb,
            0x38, 0x48, 0xc4, 0xdc,
        ];

        let d = [
            0xf1, 0x9d, 0x9b, 0x79, 0x7e, 0x39, 0xf3, 0x37, 0x44, 0x58, 0x39,
        ];

        let pk_d = [
            0xdb, 0x4c, 0xd2, 0xb0, 0xaa, 0xc4, 0xf7, 0xeb, 0x8c, 0xa1, 0x31, 0xf1, 0x65, 0x67,
            0xc4, 0x45, 0xa9, 0x55, 0x51, 0x26, 0xd3, 0xc2, 0x9f, 0x14, 0xe3, 0xd7, 0x76, 0xe8,
            0x41, 0xae, 0x74, 0x15,
        ];
        let value: u64 = 100000000;
        let rcm = [
            0x39, 0x17, 0x6d, 0xac, 0x39, 0xac, 0xe4, 0x98, 0x0e, 0xcc, 0x8d, 0x77, 0x8e, 0x89,
            0x86, 0x02, 0x55, 0xec, 0x36, 0x15, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let memo: [u8; 512] = [
            0xf6, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let encciphertext: [u8; ENC_CIPHERTEXT_SIZE] = [
            0x8d, 0x6b, 0x27, 0xe7, 0xef, 0xf5, 0x9b, 0xfb, 0xa0, 0x1d, 0x65, 0x88, 0xba, 0xdd,
            0x36, 0x6c, 0xe5, 0x9b, 0x4d, 0x5b, 0x0e, 0xf9, 0x3b, 0xeb, 0xcb, 0xf2, 0x11, 0x41,
            0x7c, 0x56, 0xae, 0x70, 0x0a, 0xe1, 0x82, 0x44, 0xba, 0xc2, 0xfb, 0x64, 0x37, 0xdb,
            0x01, 0xf8, 0x3d, 0xc1, 0x49, 0xe2, 0x78, 0x6e, 0xc4, 0xec, 0x32, 0xc1, 0x1b, 0x05,
            0x4a, 0x4c, 0x0e, 0x2b, 0xdb, 0xe3, 0x43, 0x78, 0x8b, 0xb9, 0xc3, 0x3f, 0xf4, 0x2f,
            0xae, 0x99, 0x32, 0x32, 0x13, 0xe0, 0x96, 0x3e, 0x6f, 0x97, 0x6d, 0x6f, 0xff, 0xb8,
            0xc9, 0xfc, 0xf5, 0x21, 0x95, 0x74, 0xc7, 0xa9, 0x4c, 0x0e, 0x72, 0xf6, 0x09, 0x3a,
            0xed, 0xaf, 0xe3, 0x80, 0x62, 0x1b, 0x3b, 0xa8, 0x15, 0xd2, 0xb9, 0x72, 0x40, 0xf6,
            0x77, 0xd3, 0x90, 0xf5, 0xfc, 0x5d, 0x45, 0xee, 0xff, 0x16, 0x68, 0x8e, 0x40, 0xb9,
            0xee, 0xe8, 0xee, 0x1d, 0x39, 0x3b, 0x00, 0x97, 0x50, 0xcb, 0x73, 0xdf, 0x7a, 0x47,
            0xfd, 0x07, 0xa2, 0x81, 0x41, 0xdb, 0x49, 0xbd, 0x9c, 0xca, 0xb1, 0xf1, 0x8d, 0x0b,
            0x6a, 0x55, 0xed, 0x10, 0x1c, 0xa1, 0x6f, 0x73, 0x45, 0xbc, 0xb0, 0xbe, 0xaf, 0x7c,
            0xd7, 0x9a, 0x3d, 0x2b, 0xf2, 0x88, 0xf1, 0xd8, 0x8e, 0xbb, 0x1e, 0x4b, 0x74, 0x21,
            0x99, 0xd3, 0x30, 0xc3, 0x0a, 0x9f, 0xee, 0x1b, 0x44, 0xc6, 0x86, 0xa1, 0xff, 0x5c,
            0xc3, 0x3d, 0x46, 0x27, 0xf8, 0x3d, 0x61, 0xce, 0x34, 0xd6, 0xf1, 0x34, 0x4e, 0x2b,
            0x11, 0xa5, 0xf7, 0x17, 0x24, 0x42, 0x29, 0x60, 0x75, 0x91, 0x90, 0x05, 0x43, 0x4a,
            0x57, 0x4e, 0xd4, 0xe4, 0xc9, 0x8e, 0x23, 0x8e, 0xdd, 0x53, 0x67, 0xe8, 0xf5, 0x75,
            0x24, 0xb6, 0x38, 0xdd, 0x2d, 0x58, 0x30, 0xe8, 0x3f, 0x7f, 0x32, 0x08, 0x0d, 0x2d,
            0x51, 0xa0, 0x8a, 0xe8, 0x4e, 0x37, 0x42, 0x9c, 0x84, 0x38, 0xfa, 0xae, 0x15, 0x40,
            0x86, 0x7b, 0x12, 0xac, 0x2c, 0xf6, 0xa7, 0x7d, 0xa7, 0x80, 0xd9, 0x2c, 0xfa, 0x50,
            0x0c, 0x19, 0x5a, 0x07, 0x1c, 0xe8, 0xae, 0x3f, 0x10, 0x2c, 0xe0, 0x95, 0x01, 0xec,
            0xda, 0xc0, 0x8a, 0x79, 0x52, 0xa0, 0x8d, 0x53, 0xf3, 0x62, 0xd3, 0x7b, 0x64, 0x94,
            0x8c, 0x99, 0x15, 0xcb, 0xfc, 0x9f, 0x2d, 0x3c, 0x4e, 0x82, 0x22, 0xd3, 0x9a, 0x34,
            0x84, 0x21, 0x44, 0x7f, 0xab, 0xe4, 0xd5, 0xf0, 0x87, 0x80, 0x9a, 0x79, 0xe8, 0x49,
            0xb2, 0x8d, 0xff, 0xbc, 0x97, 0xfb, 0xbf, 0x64, 0x7f, 0xf3, 0x4f, 0x79, 0xff, 0x64,
            0xe7, 0x37, 0xeb, 0xf0, 0x3d, 0x8a, 0xdd, 0x44, 0xc1, 0x54, 0x32, 0x5f, 0x2b, 0xff,
            0x14, 0xc6, 0xe9, 0xe9, 0x0b, 0x0f, 0x98, 0x89, 0xf3, 0x25, 0xa9, 0x26, 0xa3, 0x68,
            0x56, 0x41, 0xa7, 0xa2, 0x19, 0xec, 0xe6, 0xfb, 0x2b, 0x4d, 0xee, 0xbf, 0x31, 0x09,
            0xd7, 0xee, 0x0f, 0x03, 0x9d, 0xac, 0x42, 0x74, 0x44, 0x99, 0x34, 0x85, 0x84, 0x84,
            0x44, 0xcc, 0xaf, 0xda, 0x5e, 0xa3, 0x28, 0x74, 0x06, 0x66, 0xdd, 0x75, 0xc3, 0x23,
            0xce, 0x7b, 0x92, 0x0e, 0xe0, 0xf3, 0xdc, 0x3a, 0xbc, 0xe6, 0xbd, 0x09, 0xc1, 0x3c,
            0x95, 0x7c, 0x5e, 0xa8, 0x95, 0x28, 0x27, 0x11, 0x6b, 0xb5, 0xbd, 0x0e, 0x5c, 0x27,
            0xf8, 0x20, 0xf2, 0xcf, 0x72, 0xa5, 0x10, 0x5d, 0x95, 0x55, 0xbe, 0x1e, 0x1e, 0x5e,
            0x68, 0xff, 0xfb, 0x71, 0x33, 0xdc, 0x39, 0x00, 0x19, 0x4e, 0x3b, 0x73, 0x1c, 0x7d,
            0x39, 0x11, 0x70, 0xad, 0x6d, 0x4a, 0xf1, 0x3a, 0x78, 0xa0, 0x6c, 0x25, 0xcf, 0xbb,
            0x0d, 0x09, 0x91, 0xd5, 0xa8, 0x83, 0xcf, 0xf5, 0x1c, 0xb6, 0xf5, 0x91, 0xc7, 0x92,
            0xd9, 0x9d, 0xcc, 0x55, 0x9c, 0xde, 0x9b, 0x7b, 0x39, 0xc4, 0xf5, 0x4a, 0x6b, 0xfb,
            0x29, 0xf1, 0xf8, 0x5e, 0x13, 0x5d, 0x17, 0x33, 0xb4, 0x9d, 0x5d, 0xd6, 0x70, 0x18,
            0xe6, 0x2e, 0x8c, 0x1a, 0xb0, 0xc1, 0x9a, 0x25, 0x41, 0x87, 0x26, 0xcc, 0xf2, 0xf5,
            0xe8, 0x8b, 0x97, 0x69, 0x21, 0x12, 0x92, 0x4b, 0xda, 0x2f, 0xde, 0x73, 0x48, 0xba,
            0xd7, 0x29, 0x52, 0x41, 0x72, 0x9d, 0xb4, 0xf3, 0x87, 0x11, 0xc7, 0xea, 0x98, 0xc5,
            0xd4, 0x19, 0x7c, 0x66, 0xfd, 0x23,
        ];

        let c_out = encrypt_note_plaintext(&esk, &epk, &d, &pk_d, value, &rcm, &memo);
        let c_compact = encrypt_compact_plaintext(&esk, &epk, &d, &pk_d, value, &rcm);
        let x = c_out.to_vec();

        let mut cout = [0x00; 52];
        cout.copy_from_slice(&c_compact[0..52]);

        assert_eq!(x[..], encciphertext[..]);
        assert_eq!(x[0..52][..], cout[..]);
    }

    #[test]
    fn test_decrypt_note() {
        let encciphertext: [u8; ENC_CIPHERTEXT_SIZE] = [
            0x8d, 0x6b, 0x27, 0xe7, 0xef, 0xf5, 0x9b, 0xfb, 0xa0, 0x1d, 0x65, 0x88, 0xba, 0xdd,
            0x36, 0x6c, 0xe5, 0x9b, 0x4d, 0x5b, 0x0e, 0xf9, 0x3b, 0xeb, 0xcb, 0xf2, 0x11, 0x41,
            0x7c, 0x56, 0xae, 0x70, 0x0a, 0xe1, 0x82, 0x44, 0xba, 0xc2, 0xfb, 0x64, 0x37, 0xdb,
            0x01, 0xf8, 0x3d, 0xc1, 0x49, 0xe2, 0x78, 0x6e, 0xc4, 0xec, 0x32, 0xc1, 0x1b, 0x05,
            0x4a, 0x4c, 0x0e, 0x2b, 0xdb, 0xe3, 0x43, 0x78, 0x8b, 0xb9, 0xc3, 0x3f, 0xf4, 0x2f,
            0xae, 0x99, 0x32, 0x32, 0x13, 0xe0, 0x96, 0x3e, 0x6f, 0x97, 0x6d, 0x6f, 0xff, 0xb8,
            0xc9, 0xfc, 0xf5, 0x21, 0x95, 0x74, 0xc7, 0xa9, 0x4c, 0x0e, 0x72, 0xf6, 0x09, 0x3a,
            0xed, 0xaf, 0xe3, 0x80, 0x62, 0x1b, 0x3b, 0xa8, 0x15, 0xd2, 0xb9, 0x72, 0x40, 0xf6,
            0x77, 0xd3, 0x90, 0xf5, 0xfc, 0x5d, 0x45, 0xee, 0xff, 0x16, 0x68, 0x8e, 0x40, 0xb9,
            0xee, 0xe8, 0xee, 0x1d, 0x39, 0x3b, 0x00, 0x97, 0x50, 0xcb, 0x73, 0xdf, 0x7a, 0x47,
            0xfd, 0x07, 0xa2, 0x81, 0x41, 0xdb, 0x49, 0xbd, 0x9c, 0xca, 0xb1, 0xf1, 0x8d, 0x0b,
            0x6a, 0x55, 0xed, 0x10, 0x1c, 0xa1, 0x6f, 0x73, 0x45, 0xbc, 0xb0, 0xbe, 0xaf, 0x7c,
            0xd7, 0x9a, 0x3d, 0x2b, 0xf2, 0x88, 0xf1, 0xd8, 0x8e, 0xbb, 0x1e, 0x4b, 0x74, 0x21,
            0x99, 0xd3, 0x30, 0xc3, 0x0a, 0x9f, 0xee, 0x1b, 0x44, 0xc6, 0x86, 0xa1, 0xff, 0x5c,
            0xc3, 0x3d, 0x46, 0x27, 0xf8, 0x3d, 0x61, 0xce, 0x34, 0xd6, 0xf1, 0x34, 0x4e, 0x2b,
            0x11, 0xa5, 0xf7, 0x17, 0x24, 0x42, 0x29, 0x60, 0x75, 0x91, 0x90, 0x05, 0x43, 0x4a,
            0x57, 0x4e, 0xd4, 0xe4, 0xc9, 0x8e, 0x23, 0x8e, 0xdd, 0x53, 0x67, 0xe8, 0xf5, 0x75,
            0x24, 0xb6, 0x38, 0xdd, 0x2d, 0x58, 0x30, 0xe8, 0x3f, 0x7f, 0x32, 0x08, 0x0d, 0x2d,
            0x51, 0xa0, 0x8a, 0xe8, 0x4e, 0x37, 0x42, 0x9c, 0x84, 0x38, 0xfa, 0xae, 0x15, 0x40,
            0x86, 0x7b, 0x12, 0xac, 0x2c, 0xf6, 0xa7, 0x7d, 0xa7, 0x80, 0xd9, 0x2c, 0xfa, 0x50,
            0x0c, 0x19, 0x5a, 0x07, 0x1c, 0xe8, 0xae, 0x3f, 0x10, 0x2c, 0xe0, 0x95, 0x01, 0xec,
            0xda, 0xc0, 0x8a, 0x79, 0x52, 0xa0, 0x8d, 0x53, 0xf3, 0x62, 0xd3, 0x7b, 0x64, 0x94,
            0x8c, 0x99, 0x15, 0xcb, 0xfc, 0x9f, 0x2d, 0x3c, 0x4e, 0x82, 0x22, 0xd3, 0x9a, 0x34,
            0x84, 0x21, 0x44, 0x7f, 0xab, 0xe4, 0xd5, 0xf0, 0x87, 0x80, 0x9a, 0x79, 0xe8, 0x49,
            0xb2, 0x8d, 0xff, 0xbc, 0x97, 0xfb, 0xbf, 0x64, 0x7f, 0xf3, 0x4f, 0x79, 0xff, 0x64,
            0xe7, 0x37, 0xeb, 0xf0, 0x3d, 0x8a, 0xdd, 0x44, 0xc1, 0x54, 0x32, 0x5f, 0x2b, 0xff,
            0x14, 0xc6, 0xe9, 0xe9, 0x0b, 0x0f, 0x98, 0x89, 0xf3, 0x25, 0xa9, 0x26, 0xa3, 0x68,
            0x56, 0x41, 0xa7, 0xa2, 0x19, 0xec, 0xe6, 0xfb, 0x2b, 0x4d, 0xee, 0xbf, 0x31, 0x09,
            0xd7, 0xee, 0x0f, 0x03, 0x9d, 0xac, 0x42, 0x74, 0x44, 0x99, 0x34, 0x85, 0x84, 0x84,
            0x44, 0xcc, 0xaf, 0xda, 0x5e, 0xa3, 0x28, 0x74, 0x06, 0x66, 0xdd, 0x75, 0xc3, 0x23,
            0xce, 0x7b, 0x92, 0x0e, 0xe0, 0xf3, 0xdc, 0x3a, 0xbc, 0xe6, 0xbd, 0x09, 0xc1, 0x3c,
            0x95, 0x7c, 0x5e, 0xa8, 0x95, 0x28, 0x27, 0x11, 0x6b, 0xb5, 0xbd, 0x0e, 0x5c, 0x27,
            0xf8, 0x20, 0xf2, 0xcf, 0x72, 0xa5, 0x10, 0x5d, 0x95, 0x55, 0xbe, 0x1e, 0x1e, 0x5e,
            0x68, 0xff, 0xfb, 0x71, 0x33, 0xdc, 0x39, 0x00, 0x19, 0x4e, 0x3b, 0x73, 0x1c, 0x7d,
            0x39, 0x11, 0x70, 0xad, 0x6d, 0x4a, 0xf1, 0x3a, 0x78, 0xa0, 0x6c, 0x25, 0xcf, 0xbb,
            0x0d, 0x09, 0x91, 0xd5, 0xa8, 0x83, 0xcf, 0xf5, 0x1c, 0xb6, 0xf5, 0x91, 0xc7, 0x92,
            0xd9, 0x9d, 0xcc, 0x55, 0x9c, 0xde, 0x9b, 0x7b, 0x39, 0xc4, 0xf5, 0x4a, 0x6b, 0xfb,
            0x29, 0xf1, 0xf8, 0x5e, 0x13, 0x5d, 0x17, 0x33, 0xb4, 0x9d, 0x5d, 0xd6, 0x70, 0x18,
            0xe6, 0x2e, 0x8c, 0x1a, 0xb0, 0xc1, 0x9a, 0x25, 0x41, 0x87, 0x26, 0xcc, 0xf2, 0xf5,
            0xe8, 0x8b, 0x97, 0x69, 0x21, 0x12, 0x92, 0x4b, 0xda, 0x2f, 0xde, 0x73, 0x48, 0xba,
            0xd7, 0x29, 0x52, 0x41, 0x72, 0x9d, 0xb4, 0xf3, 0x87, 0x11, 0xc7, 0xea, 0x98, 0xc5,
            0xd4, 0x19, 0x7c, 0x66, 0xfd, 0x23,
        ];

        let ivk = [
            0xb7, 0x0b, 0x7c, 0xd0, 0xed, 0x03, 0xcb, 0xdf, 0xd7, 0xad, 0xa9, 0x50, 0x2e, 0xe2,
            0x45, 0xb1, 0x3e, 0x56, 0x9d, 0x54, 0xa5, 0x71, 0x9d, 0x2d, 0xaa, 0x0f, 0x5f, 0x14,
            0x51, 0x47, 0x92, 0x04,
        ];

        let epk = [
            0xde, 0xd6, 0x8f, 0x05, 0xc6, 0x58, 0xfc, 0xae, 0x5a, 0xe2, 0x18, 0x64, 0x6f, 0xf8,
            0x44, 0x40, 0x6f, 0x84, 0x42, 0x67, 0x84, 0x04, 0x0d, 0x0b, 0xef, 0x2b, 0x09, 0xcb,
            0x38, 0x48, 0xc4, 0xdc,
        ];

        let cmu = [
            0x63, 0x55, 0x72, 0xf5, 0x72, 0xa8, 0xa1, 0xa0, 0xb7, 0xac, 0xbc, 0x0a, 0xfc, 0x6d,
            0x66, 0xf1, 0x4a, 0x02, 0xef, 0xac, 0xde, 0x7b, 0xdf, 0x03, 0x44, 0x3e, 0xd4, 0xc3,
            0xe5, 0x51, 0xd4, 0x70,
        ];

        let t = try_sapling_note_decryption(&ivk, &epk, &cmu, &encciphertext);
        assert!(t);
    }

    #[test]
    fn test_compact_encryption() {
        let esk = [
            0x81, 0xc7, 0xb2, 0x17, 0x1f, 0xf4, 0x41, 0x52, 0x50, 0xca, 0xc0, 0x1f, 0x59, 0x82,
            0xfd, 0x8f, 0x49, 0x61, 0x9d, 0x61, 0xad, 0x78, 0xf6, 0x83, 0x0b, 0x3c, 0x60, 0x61,
            0x45, 0x96, 0x2a, 0x0e,
        ];
        let epk = [
            0xde, 0xd6, 0x8f, 0x05, 0xc6, 0x58, 0xfc, 0xae, 0x5a, 0xe2, 0x18, 0x64, 0x6f, 0xf8,
            0x44, 0x40, 0x6f, 0x84, 0x42, 0x67, 0x84, 0x04, 0x0d, 0x0b, 0xef, 0x2b, 0x09, 0xcb,
            0x38, 0x48, 0xc4, 0xdc,
        ];

        let d = [
            0xf1, 0x9d, 0x9b, 0x79, 0x7e, 0x39, 0xf3, 0x37, 0x44, 0x58, 0x39,
        ];

        let pk_d = [
            0xdb, 0x4c, 0xd2, 0xb0, 0xaa, 0xc4, 0xf7, 0xeb, 0x8c, 0xa1, 0x31, 0xf1, 0x65, 0x67,
            0xc4, 0x45, 0xa9, 0x55, 0x51, 0x26, 0xd3, 0xc2, 0x9f, 0x14, 0xe3, 0xd7, 0x76, 0xe8,
            0x41, 0xae, 0x74, 0x15,
        ];
        let value: u64 = 100000000;
        let rcm = [
            0x39, 0x17, 0x6d, 0xac, 0x39, 0xac, 0xe4, 0x98, 0x0e, 0xcc, 0x8d, 0x77, 0x8e, 0x89,
            0x86, 0x02, 0x55, 0xec, 0x36, 0x15, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let enctxt: [u8; 52] = [
            0x8d, 0x6b, 0x27, 0xe7, 0xef, 0xf5, 0x9b, 0xfb, 0xa0, 0x1d, 0x65, 0x88, 0xba, 0xdd,
            0x36, 0x6c, 0xe5, 0x9b, 0x4d, 0x5b, 0x0e, 0xf9, 0x3b, 0xeb, 0xcb, 0xf2, 0x11, 0x41,
            0x7c, 0x56, 0xae, 0x70, 0x0a, 0xe1, 0x82, 0x44, 0xba, 0xc2, 0xfb, 0x64, 0x37, 0xdb,
            0x01, 0xf8, 0x3d, 0xc1, 0x49, 0xe2, 0x78, 0x6e, 0xc4, 0xec,
        ];

        let c_out = encrypt_compact_plaintext(&esk, &epk, &d, &pk_d, value, &rcm);
        let x = c_out.to_vec();
        assert_eq!(x[0..52][..], enctxt[..]);
    }

    #[test]
    fn test_compact_decryption() {
        let encciphertext: [u8; ENC_COMPACT_SIZE] = [
            0x8d, 0x6b, 0x27, 0xe7, 0xef, 0xf5, 0x9b, 0xfb, 0xa0, 0x1d, 0x65, 0x88, 0xba, 0xdd,
            0x36, 0x6c, 0xe5, 0x9b, 0x4d, 0x5b, 0x0e, 0xf9, 0x3b, 0xeb, 0xcb, 0xf2, 0x11, 0x41,
            0x7c, 0x56, 0xae, 0x70, 0x0a, 0xe1, 0x82, 0x44, 0xba, 0xc2, 0xfb, 0x64, 0x37, 0xdb,
            0x01, 0xf8, 0x3d, 0xc1, 0x49, 0xe2, 0x78, 0x6e, 0xc4, 0xec, 204, 147, 70, 213, 254, 89,
            162, 103, 211, 121, 128, 122, 14, 85, 117, 199,
        ];

        let ivk = [
            0xb7, 0x0b, 0x7c, 0xd0, 0xed, 0x03, 0xcb, 0xdf, 0xd7, 0xad, 0xa9, 0x50, 0x2e, 0xe2,
            0x45, 0xb1, 0x3e, 0x56, 0x9d, 0x54, 0xa5, 0x71, 0x9d, 0x2d, 0xaa, 0x0f, 0x5f, 0x14,
            0x51, 0x47, 0x92, 0x04,
        ];

        let epk = [
            0xde, 0xd6, 0x8f, 0x05, 0xc6, 0x58, 0xfc, 0xae, 0x5a, 0xe2, 0x18, 0x64, 0x6f, 0xf8,
            0x44, 0x40, 0x6f, 0x84, 0x42, 0x67, 0x84, 0x04, 0x0d, 0x0b, 0xef, 0x2b, 0x09, 0xcb,
            0x38, 0x48, 0xc4, 0xdc,
        ];

        let cmu = [
            0x63, 0x55, 0x72, 0xf5, 0x72, 0xa8, 0xa1, 0xa0, 0xb7, 0xac, 0xbc, 0x0a, 0xfc, 0x6d,
            0x66, 0xf1, 0x4a, 0x02, 0xef, 0xac, 0xde, 0x7b, 0xdf, 0x03, 0x44, 0x3e, 0xd4, 0xc3,
            0xe5, 0x51, 0xd4, 0x70,
        ];

        let t = try_sapling_compact_decryption(&ivk, &epk, &cmu, &encciphertext);
        assert!(t);
    }
}
