use core::convert::TryInto;
use core::mem;

use crate::constants;
use crate::{bolos, zip32};

use crate::bolos::c_zemu_log_stack;
use blake2s_simd::{blake2s, Hash as Blake2sHash, Params as Blake2sParams};
use jubjub::{AffineNielsPoint, AffinePoint, ExtendedPoint, Fq, Fr};

#[inline(always)]
pub fn prf_expand(sk: &[u8], t: &[u8]) -> [u8; 64] {
    bolos::blake2b_expand_seed(sk, t)
}

fn sapling_derive_dummy_ask(sk_in: &[u8]) -> [u8; 32] {
    let t = prf_expand(&sk_in, &[0x00]);
    let ask = Fr::from_bytes_wide(&t);
    ask.to_bytes()
}

fn sapling_derive_dummy_nsk(sk_in: &[u8]) -> [u8; 32] {
    let t = prf_expand(&sk_in, &[0x01]);
    let nsk = Fr::from_bytes_wide(&t);
    nsk.to_bytes()
}

fn sapling_ask_to_ak(ask: &[u8; 32]) -> [u8; 32] {
    let ak = constants::SPENDING_KEY_BASE.multiply_bits(&ask);
    AffinePoint::from(ak).to_bytes()
}

fn sapling_nsk_to_nk(nsk: &[u8; 32]) -> [u8; 32] {
    let nk = constants::PROVING_KEY_BASE.multiply_bits(&nsk);
    AffinePoint::from(nk).to_bytes()
}

fn aknk_to_ivk(ak: &[u8; 32], nk: &[u8; 32]) -> [u8; 32] {
    pub const CRH_IVK_PERSONALIZATION: &[u8; 8] = b"Zcashivk"; //move to constants

    // blake2s CRH_IVK_PERSONALIZATION || ak || nk
    let h = Blake2sParams::new()
        .hash_length(32)
        .personal(CRH_IVK_PERSONALIZATION)
        .to_state()
        .update(ak)
        .update(nk)
        .finalize();

    let mut x: [u8; 32] = *h.as_array();
    x[31] &= 0b0000_0111; //check this
    x
}

#[inline(never)]
fn diversifier_group_hash_check(hash: &[u8; 32]) -> bool {
    let u = AffinePoint::from_bytes(*hash);
    if u.is_some().unwrap_u8() == 1 {
        let v = u.unwrap();
        let q = v.mul_by_cofactor();
        let i = ExtendedPoint::identity();
        return q != i;
    }

    false
}

#[inline(never)]
fn diversifier_group_hash_light(tag: &[u8]) -> bool {
    let x = bolos::blake2s_diversification(tag);

    //    diversifier_group_hash_check(&x)

    let u = AffinePoint::from_bytes(x);
    if u.is_some().unwrap_u8() == 1 {
        let v = u.unwrap();
        let q = v.mul_by_cofactor();
        let i = ExtendedPoint::identity();
        return q != i;
    }

    false
}

#[inline(never)]
fn default_diversifier(sk: &[u8; 32]) -> [u8; 11] {
    //fixme: replace blake2b with aes
    let mut c: [u8; 2] = [0x03, 0x0];

    // blake2b sk || 0x03 || c
    loop {
        let x = prf_expand(sk, &c);
        if diversifier_group_hash_light(&x[0..11]) {
            let mut result = [0u8; 11];
            result.copy_from_slice(&x[..11]);
            return result;
        }
        c[1] += 1;
    }
}

#[inline(never)]
fn pkd_group_hash(d: &[u8; 11]) -> [u8; 32] {
    let h = bolos::blake2s_diversification(d);

    let v = AffinePoint::from_bytes(h).unwrap();
    let q = v.mul_by_cofactor();
    let t = AffinePoint::from(q);
    t.to_bytes()
}

#[inline(never)]
fn default_pkd(ivk: &[u8; 32], d: &[u8; 11]) -> [u8; 32] {
    let h = bolos::blake2s_diversification(d);

    let v = AffinePoint::from_bytes(h).unwrap();
    let y = v.mul_by_cofactor();

    // FIXME: We should avoid asserts in ledger code
    //assert_eq!(x.is_some().unwrap_u8(), 1);

    let v = y.to_niels().multiply_bits(ivk);
    let t = AffinePoint::from(v);
    t.to_bytes()
}

#[no_mangle]
pub extern "C" fn ask_to_ak(ask_ptr: *const u8, ak_ptr: *mut u8) {
    let ask: &[u8; 32] = unsafe { mem::transmute(ask_ptr) };
    let ak: &mut [u8; 32] = unsafe { mem::transmute(ak_ptr) };
    let tmp_ak = zip32::sapling_ask_to_ak(&ask);
    ak.copy_from_slice(&tmp_ak)
}

#[no_mangle]
pub extern "C" fn nsk_to_nk(nsk_ptr: *const u8, nk_ptr: *mut u8) {
    let nsk = unsafe { &*(nsk_ptr as *const [u8; 32]) };
    let nk: &mut [u8; 32] = unsafe { mem::transmute(nk_ptr) };
    let tmp_nk = zip32::sapling_nsk_to_nk(&nsk);
    nk.copy_from_slice(&tmp_nk)
}

#[no_mangle]
pub extern "C" fn get_ak(sk_ptr: *const u8, ak_ptr: *mut u8) {
    let sk: &[u8; 32] = unsafe { mem::transmute(sk_ptr) };
    let ak: &mut [u8; 32] = unsafe { mem::transmute(ak_ptr) };
    let ask = zip32::sapling_derive_dummy_ask(sk);
    let tmp_ak = zip32::sapling_ask_to_ak(&ask);
    ak.copy_from_slice(&tmp_ak)
}

#[no_mangle]
pub extern "C" fn get_nk(sk_ptr: *const u8, nk_ptr: *mut u8) {
    let sk: &[u8; 32] = unsafe { mem::transmute(sk_ptr) };
    let nk: &mut [u8; 32] = unsafe { mem::transmute(nk_ptr) };
    let nsk = zip32::sapling_derive_dummy_nsk(sk);
    let tmp_nk = zip32::sapling_nsk_to_nk(&nsk);
    nk.copy_from_slice(&tmp_nk)
}

#[no_mangle]
pub extern "C" fn get_ivk(ak_ptr: *const u8, nk_ptr: *mut u8, ivk_ptr: *mut u8) {
    let ak: &[u8; 32] = unsafe { mem::transmute(ak_ptr) };
    let nk: &[u8; 32] = unsafe { mem::transmute(nk_ptr) };
    let ivk: &mut [u8; 32] = unsafe { mem::transmute(ivk_ptr) };

    let tmp_ivk = zip32::aknk_to_ivk(&ak, &nk);
    ivk.copy_from_slice(&tmp_ivk)
}

#[no_mangle]
pub extern "C" fn zip32_master(seed_ptr: *const u8, sk_ptr: *mut u8, dk_ptr: *mut u8) {
    let seed: &[u8; 32] = unsafe { mem::transmute(seed_ptr) };
    let sk: &mut [u8; 32] = unsafe { mem::transmute(sk_ptr) };
    let dk: &mut [u8; 32] = unsafe { mem::transmute(dk_ptr) };

    let k = zip32::derive_zip32_master(seed);
    sk.copy_from_slice(&k[0..32]);
    dk.copy_from_slice(&k[32..64])
}

//fixme
#[no_mangle]
pub extern "C" fn zip32_child(
    seed_ptr: *const u8,
    dk_ptr: *mut u8,
    ask_ptr: *mut u8,
    nsk_ptr: *mut u8,
) {
    let seed: &[u8; 32] = unsafe { mem::transmute(seed_ptr) };
    let dk: &mut [u8; 32] = unsafe { mem::transmute(dk_ptr) };
    let ask: &mut [u8; 32] = unsafe { mem::transmute(ask_ptr) };
    let nsk: &mut [u8; 32] = unsafe { mem::transmute(nsk_ptr) };
    let p: u32 = 0x80000001;
    let k = zip32::derive_zip32_child_fromseedandpath(seed, &[p]); //todo: fix me
    dk.copy_from_slice(&k[0..32]);
    ask.copy_from_slice(&k[32..64]);
    nsk.copy_from_slice(&k[64..96]);
}

#[no_mangle]
pub extern "C" fn get_diversifier(sk_ptr: *mut u8, diversifier_ptr: *mut u8) {
    let sk: &[u8; 32] = unsafe { mem::transmute::<*const u8, &[u8; 32]>(sk_ptr) };
    let diversifier: &mut [u8; 11] =
        unsafe { mem::transmute::<*const u8, &mut [u8; 11]>(diversifier_ptr) };
    let d = default_diversifier(sk);
    diversifier.copy_from_slice(&d)
}

#[no_mangle]
pub extern "C" fn get_diversifier_list(sk_ptr: *const u8, diversifier_list_ptr: *mut u8) {
    let sk: &[u8; 32] = unsafe { mem::transmute(sk_ptr) };
    let diversifier: &mut [u8; 44] = unsafe { mem::transmute(diversifier_list_ptr) };
    let d = zip32::ff1aes_list(sk);
    diversifier.copy_from_slice(&d)
}

#[no_mangle]
pub extern "C" fn get_diversifier_fromlist(div_ptr: *mut u8, diversifier_list_ptr: *const u8) {
    let diversifier_list: &mut [u8; 44] = unsafe { mem::transmute(diversifier_list_ptr) };
    let div: &mut [u8; 11] = unsafe { mem::transmute(div_ptr) };

    let d = zip32::default_diversifier_fromlist(diversifier_list);
    div.copy_from_slice(&d)
}

#[no_mangle]
pub extern "C" fn get_pkd(ivk_ptr: *mut u8, diversifier_ptr: *mut u8, pkd_ptr: *mut u8) {
    let ivk: &[u8; 32] = unsafe { mem::transmute(ivk_ptr) };
    let diversifier: &[u8; 11] = unsafe { mem::transmute(diversifier_ptr) };
    let pkd: &mut [u8; 32] = unsafe { mem::transmute(pkd_ptr) };

    let tmp_pkd = zip32::default_pkd(&ivk, &diversifier);
    pkd.copy_from_slice(&tmp_pkd)
}

//fixme
//fixme: we need to add a prefix to exported functions.. as there are no namespaces in C :(
//get seed from the ledger
#[no_mangle]
pub extern "C" fn get_address(sk_ptr: *mut u8, ivk_ptr: *mut u8, address_ptr: *mut u8) {
    let sk: &[u8; 32] = unsafe { mem::transmute::<*const u8, &[u8; 32]>(sk_ptr) };
    let ivk: &[u8; 32] = unsafe { mem::transmute::<*const u8, &[u8; 32]>(ivk_ptr) };
    let address: &mut [u8; 43] = unsafe { mem::transmute::<*const u8, &mut [u8; 43]>(address_ptr) };

    let div = default_diversifier(sk);
    let pkd = default_pkd(&ivk, &div);

    address[..11].copy_from_slice(&div);
    address[11..].copy_from_slice(&pkd);
}

#[inline(never)]
fn group_hash_check(hash: &[u8; 32]) -> bool {
    let u = AffinePoint::from_bytes(*hash);
    if u.is_some().unwrap_u8() == 1 {
        let v = u.unwrap();
        let q = v.mul_by_cofactor();
        let i = ExtendedPoint::identity();
        return q != i;
    }

    false
}

//use crypto_api_chachapoly::{ChaCha20Ietf, ChachaPolyIetf};
//use subtle::ConditionallySelectable; //TODO: replace me with no-std version

const COMPACT_NOTE_SIZE: usize = 1 /* version */ + 11 /*diversifier*/ + 8 /*value*/ + 32 /*rcv*/;

const NOTE_PLAINTEXT_SIZE: usize = COMPACT_NOTE_SIZE + 512;
const OUT_PLAINTEXT_SIZE: usize = 32 /*pk_d*/ + 32 /* esk */;

const ENC_CIPHERTEXT_SIZE: usize = NOTE_PLAINTEXT_SIZE + 16;
const OUT_CIPHERTEXT_SIZE: usize = OUT_PLAINTEXT_SIZE + 16;

pub fn generate_esk(buffer: [u8; 64]) -> [u8; 32] {
    //Rng.fill_bytes(&mut buffer); fill with random bytes
    let esk = Fr::from_bytes_wide(&buffer);
    esk.to_bytes()
}

pub fn derive_public(esk: [u8; 32], g_d: [u8; 32]) -> [u8; 32] {
    let p = AffinePoint::from_bytes(g_d).unwrap();
    let q = p.to_niels().multiply_bits(&esk);
    let t = AffinePoint::from(q);
    t.to_bytes()
}

pub fn sapling_ka_agree(esk: [u8; 32], pk_d: [u8; 32]) -> [u8; 32] {
    let p = AffinePoint::from_bytes(pk_d).unwrap();
    let q = p.mul_by_cofactor();
    let v = q.to_niels().multiply_bits(&esk);
    let t = AffinePoint::from(v);
    t.to_bytes()
}

fn kdf_sapling(dhsecret: [u8; 32], epk: [u8; 32]) -> [u8; 32] {
    let mut input = [0u8; 64];
    (&mut input[..32]).copy_from_slice(&dhsecret);
    (&mut input[32..]).copy_from_slice(&epk);
    bolos::blake2b_kdf_sapling(&input)
}

fn prf_ock(ovk: [u8; 32], cv: [u8; 32], cmu: [u8; 32], epk: [u8; 32]) -> [u8; 32] {
    let mut ock_input = [0u8; 128];
    ock_input[0..32].copy_from_slice(&ovk); //Todo: compute this from secret key
    ock_input[32..64].copy_from_slice(&cv);
    ock_input[64..96].copy_from_slice(&cmu);
    ock_input[96..128].copy_from_slice(&epk);

    bolos::blake2b_prf_ock(&ock_input)
}

/*
fn chacha_encryptnote(
    key: [u8; 32],
    plaintext: [u8; NOTE_PLAINTEXT_SIZE],
) -> [u8; ENC_CIPHERTEXT_SIZE] {
    let mut output = [0u8; ENC_CIPHERTEXT_SIZE];
    ChachaPolyIetf::aead_cipher()
        .seal_to(&mut output, &plaintext, &[], &key, &[0u8; 12])
        .unwrap();
    output
}

fn chacha_decryptnote(
    key: [u8; 32],
    ciphertext: [u8; ENC_CIPHERTEXT_SIZE],
) -> [u8; ENC_CIPHERTEXT_SIZE] {
    let mut plaintext = [0u8; ENC_CIPHERTEXT_SIZE];
    ChachaPolyIetf::aead_cipher()
        .open_to(&mut plaintext, &ciphertext, &[], &key, &[0u8; 12])
        .unwrap();
    plaintext
}
*/
//#[inline(never)]
fn handle_chunk(bits: u8, cur: &mut Fr) -> Fr {
    let c = bits & 1;
    let b = bits & 2;
    let a = bits & 4;
    let mut tmp = *cur;
    if a == 4 {
        tmp = tmp.add(cur);
    }
    *cur = cur.double(); // 2^1 * cur
    if b == 2 {
        tmp = tmp.add(cur);
    }
    // conditionally negate
    if c == 1 {
        tmp = tmp.neg();
    }
    return tmp;
}

//assumption here that ceil(bitsize / 8) == m.len(), so appended with zero bits to fill the bytes
//#[inline(never)]
fn pedersen_hash(m: &[u8], bitsize: u64) -> [u8; 32] {
    c_zemu_log_stack(b"pedersen_hash\x00".as_ref());

    let points = [
        [
            0xca, 0x3c, 0x24, 0x32, 0xd4, 0xab, 0xbf, 0x77, 0x32, 0x46, 0x4e, 0xc0, 0x8b, 0x2e,
            0x47, 0xf9, 0x5e, 0xdc, 0x7e, 0x83, 0x6b, 0x16, 0xc9, 0x79, 0x57, 0x1b, 0x52, 0xd3,
            0xa2, 0x87, 0x9e, 0xa8,
        ],
        [
            0x91, 0x18, 0xbf, 0x4e, 0x3c, 0xc5, 0x0d, 0x7b, 0xe8, 0xd3, 0xfa, 0x98, 0xeb, 0xbe,
            0x3a, 0x1f, 0x25, 0xd9, 0x01, 0xc0, 0x42, 0x11, 0x89, 0xf7, 0x33, 0xfe, 0x43, 0x5b,
            0x7f, 0x8c, 0x5d, 0x01,
        ],
        [
            0x57, 0xd4, 0x93, 0x97, 0x2c, 0x50, 0xed, 0x80, 0x98, 0xb4, 0x84, 0x17, 0x7f, 0x2a,
            0xb2, 0x8b, 0x53, 0xe8, 0x8c, 0x8e, 0x6c, 0xa4, 0x00, 0xe0, 0x9e, 0xee, 0x4e, 0xd2,
            0x00, 0x15, 0x2e, 0xb6,
        ],
        [
            0xe9, 0x70, 0x35, 0xa3, 0xec, 0x4b, 0x71, 0x84, 0x85, 0x6a, 0x1f, 0xa1, 0xa1, 0xaf,
            0x03, 0x51, 0xb7, 0x47, 0xd9, 0xd8, 0xcb, 0x0a, 0x07, 0x91, 0xd8, 0xca, 0x56, 0x4b,
            0x0c, 0xe4, 0x7e, 0x2f,
        ],
        [
            0xef, 0x8a, 0x65, 0xc3, 0x99, 0x82, 0x96, 0x99, 0x4c, 0xd1, 0x59, 0x58, 0x09, 0xd8,
            0xb9, 0xb3, 0xe5, 0xc9, 0x06, 0x14, 0x38, 0x32, 0x78, 0x39, 0x0a, 0x9d, 0xab, 0x03,
            0x21, 0xc5, 0x4b, 0xc9,
        ],
        [
            0x9a, 0x62, 0x8d, 0x9f, 0x11, 0x82, 0x60, 0x43, 0xa7, 0x13, 0x6b, 0xc6, 0xd2, 0x00,
            0x02, 0xa8, 0x28, 0x6a, 0x13, 0x0a, 0x07, 0xb1, 0xcd, 0x64, 0xe5, 0xb6, 0xbf, 0xe8,
            0x89, 0x46, 0xec, 0xe4,
        ],
    ];

    let mut i = 0;
    let mut counter: usize = 0;
    let mut pointcounter: usize = 0;
    let maxcounter: usize = 63;
    let mut remainingbits = bitsize;

    let mut x: u64 = 0;

    let mut acc = Fr::zero();
    let mut cur = Fr::one();
    let mut result_point = ExtendedPoint::identity();

    let mut rem: u64 = 0;
    let mut k = 1;
    while i < m.len() {
        x = 0;
        //take 6 bytes = 48 bits or less, depending on remaining length
        rem = if i + 6 <= m.len() {
            6
        } else {
            (m.len() - i) as u64
        };
        x += m[i] as u64;
        i += 1;
        let mut j = 1;
        //fill x with bytes
        while j < rem {
            x <<= 8;
            x += m[i] as u64;
            i += 1;
            j += 1;
        }
        let el;
        if i == m.len() {
            //handling last bytes
            remainingbits %= 48;
            el = remainingbits / 3;
            remainingbits %= 3;
        } else {
            el = 16;
        }
        k = 1;
        while k < (el + 1) {
            let bits = (x >> (rem * 8 - k * 3) & 7) as u8;
            let tmp = handle_chunk(bits, &mut cur);
            acc = acc.add(&tmp);

            counter += 1;
            //check if we need to move to the next curvepoint
            if counter == maxcounter {
                let str = points[pointcounter];
                let q = AffinePoint::from_bytes(str).unwrap().to_niels();
                let p = q.multiply_bits(&acc.to_bytes());
                result_point = result_point + p;

                counter = 0;
                pointcounter += 1;
                acc = Fr::zero();
                cur = Fr::one();
            } else {
                cur = cur.double();
                cur = cur.double();
                cur = cur.double();
            }
            k += 1;
        }
    }
    //handle remaining bits if there are any
    if remainingbits > 0 {
        let bits: u8;
        if rem * 8 < k * 3 {
            let tr = if rem % 3 == 1 { 3 } else { 1 };
            bits = ((x & tr) << (rem % 3)) as u8;
        } else {
            bits = (x >> (rem * 8 - k * 3) & 7) as u8;
        }
        let tmp = handle_chunk(bits, &mut cur);
        acc = acc.add(&tmp);
        counter += 1;
    }
    //multiply with curve point if needed
    if counter > 0 {
        let str = points[pointcounter];
        let q = AffinePoint::from_bytes(str).unwrap().to_niels();
        let p = q.multiply_bits(&acc.to_bytes());
        result_point = result_point + p;
    }
    return AffinePoint::from(result_point).get_u().to_bytes();
}

//assume that encoding of bits is done before this
//todo: encode length?
#[no_mangle]
pub extern "C" fn do_pedersen_hash(input_ptr: *const u8, output_ptr: *mut u8) {
    c_zemu_log_stack(b"do_pedersen_hash\x00".as_ref());

    let input_msg: &[u8; 1] = unsafe { mem::transmute(input_ptr) };
    let output_msg: &mut [u8; 32] = unsafe { mem::transmute(output_ptr) };
    let h = pedersen_hash(input_msg.as_ref(), 3);
    output_msg.copy_from_slice(&h);
}

#[cfg(test)]
mod tests {
    use crate::zeccrypto::default_diversifier;
    use crate::zip32::*;
    use crate::*;
    use core::convert::TryInto;

    #[test]
    fn test_zip32_master() {
        let seed = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ];

        let dk: [u8; 32] = [
            0x77, 0xc1, 0x7c, 0xb7, 0x5b, 0x77, 0x96, 0xaf, 0xb3, 0x9f, 0x0f, 0x3e, 0x91, 0xc9,
            0x24, 0x60, 0x7d, 0xa5, 0x6f, 0xa9, 0xa2, 0x0e, 0x28, 0x35, 0x09, 0xbc, 0x8a, 0x3e,
            0xf9, 0x96, 0xa1, 0x72,
        ];
        let keys = derive_zip32_master(&seed);
        assert_eq!(keys[0..32], dk);
    }

    #[test]
    fn test_zip32_childaddress() {
        let seed = [0u8; 32];

        let p: u32 = 0x80000001;
        let keys = derive_zip32_child_fromseedandpath(&seed, &[p]);

        let mut dk = [0u8; 32];
        dk.copy_from_slice(&keys[0..32]);

        let mut ask = [0u8; 32];
        ask.copy_from_slice(&keys[32..64]);

        let mut nsk = [0u8; 32];
        nsk.copy_from_slice(&keys[64..96]);

        //fixme: add ecc operations
        let ask_test: [u8; 32] = [
            0x66, 0x5e, 0xd6, 0xf7, 0xb7, 0x93, 0xaf, 0xa1, 0x82, 0x21, 0xe1, 0x57, 0xba, 0xd5,
            0x43, 0x3c, 0x54, 0x23, 0xf4, 0xfe, 0xc9, 0x46, 0xe0, 0x8e, 0xd6, 0x30, 0xa0, 0xc6,
            0x0a, 0x1f, 0xac, 0x02,
        ];

        assert_eq!(ask, ask_test);

        let nk: [u8; 32] = sapling_nsk_to_nk(&nsk);
        let ak: [u8; 32] = sapling_ask_to_ak(&ask);

        let ivk_test: [u8; 32] = [
            0x2c, 0x57, 0xfb, 0x12, 0x8c, 0x35, 0xa4, 0x4d, 0x2d, 0x5b, 0xf2, 0xfd, 0x21, 0xdc,
            0x3b, 0x44, 0x11, 0x4c, 0x36, 0x6c, 0x9c, 0x49, 0x60, 0xc4, 0x91, 0x66, 0x17, 0x38,
            0x3e, 0x89, 0xfd, 0x00,
        ];
        let ivk = aknk_to_ivk(&ak, &nk);

        assert_eq!(ivk, ivk_test);

        let list = ff1aes_list(&dk);
        let default_d = default_diversifier_fromlist(&list);

        let pk_d = default_pkd(&ivk, &default_d);

        assert_eq!(
            default_d,
            [0x10, 0xaa, 0x8e, 0xe1, 0xe1, 0x91, 0x48, 0xe7, 0x49, 0x7d, 0x3c]
        );
        assert_eq!(
            pk_d,
            [
                0xb3, 0xbe, 0x9e, 0xb3, 0xe7, 0xa9, 0x61, 0x17, 0x95, 0x17, 0xae, 0x28, 0xab, 0x19,
                0xb4, 0x84, 0xae, 0x17, 0x2f, 0x1f, 0x33, 0xd1, 0x16, 0x33, 0xe9, 0xec, 0x05, 0xee,
                0xa1, 0xe8, 0xa9, 0xd6
            ]
        );
    }

    #[test]
    fn test_zip32_childaddress_ledgerkey() {
        let s = hex::decode("b08e3d98da431cef4566a13c1bb348b982f7d8e743b43bb62557ba51994b1257")
            .expect("error");
        let seed: [u8; 32] = s.as_slice().try_into().expect("er");
        let p: u32 = 0x80000001;
        let keys = derive_zip32_child_fromseedandpath(&seed, &[p]);

        let mut dk = [0u8; 32];
        dk.copy_from_slice(&keys[0..32]);

        let mut ask = [0u8; 32];
        ask.copy_from_slice(&keys[32..64]);

        let mut nsk = [0u8; 32];
        nsk.copy_from_slice(&keys[64..96]);

        let nk: [u8; 32] = sapling_nsk_to_nk(&nsk);
        let ak: [u8; 32] = sapling_ask_to_ak(&ask);

        let ivk = aknk_to_ivk(&ak, &nk);

        let list = ff1aes_list(&dk);
        let default_d = default_diversifier_fromlist(&list);

        let pk_d = default_pkd(&ivk, &default_d);

        assert_eq!(
            default_d,
            [250, 115, 180, 200, 239, 11, 123, 73, 187, 60, 148]
        );
        assert_eq!(
            pk_d,
            [
                191, 46, 29, 241, 178, 127, 191, 115, 187, 149, 153, 207, 116, 119, 20, 209, 250,
                139, 59, 242, 251, 143, 230, 0, 172, 160, 16, 248, 117, 182, 234, 83
            ]
        );
    }

    #[test]
    fn test_zip32_master_address_ledgerkey() {
        let s = hex::decode("b08e3d98da431cef4566a13c1bb348b982f7d8e743b43bb62557ba51994b1257")
            .expect("error");
        let seed: [u8; 32] = s.as_slice().try_into().expect("er");

        let keys = derive_zip32_master(&seed);

        let mut dk = [0u8; 32];
        dk.copy_from_slice(&keys[0..32]);

        let mut ask = [0u8; 32];
        ask.copy_from_slice(&keys[32..64]);

        let mut nsk = [0u8; 32];
        nsk.copy_from_slice(&keys[64..96]);

        let nk: [u8; 32] = sapling_nsk_to_nk(&nsk);
        let ak: [u8; 32] = sapling_ask_to_ak(&ask);

        let ivk = aknk_to_ivk(&ak, &nk);

        let list = ff1aes_list(&dk);
        let default_d = default_diversifier_fromlist(&list);

        let pk_d = default_pkd(&ivk, &default_d);

        assert_eq!(
            default_d,
            [249, 61, 207, 226, 4, 114, 83, 238, 188, 23, 212]
        );
        assert_eq!(
            pk_d,
            [
                220, 53, 23, 146, 73, 107, 157, 1, 78, 98, 108, 59, 201, 41, 230, 211, 47, 80, 127,
                184, 11, 102, 79, 92, 174, 151, 211, 123, 247, 66, 219, 169
            ]
        );
    }

    #[test]
    fn test_zip32_master_address_allzero() {
        let seed = [0u8; 32];

        let keys = derive_zip32_master(&seed);

        let mut dk = [0u8; 32];
        dk.copy_from_slice(&keys[0..32]);

        let mut ask = [0u8; 32];
        ask.copy_from_slice(&keys[32..64]);

        let mut nsk = [0u8; 32];
        nsk.copy_from_slice(&keys[64..96]);

        let nk: [u8; 32] = sapling_nsk_to_nk(&nsk);
        let ak: [u8; 32] = sapling_ask_to_ak(&ask);

        let ivk = aknk_to_ivk(&ak, &nk);

        let list = ff1aes_list(&dk);
        let default_d = default_diversifier_fromlist(&list);

        let pk_d = default_pkd(&ivk, &default_d);

        assert_eq!(
            default_d,
            [0x3b, 0xf6, 0xfa, 0x1f, 0x83, 0xbf, 0x45, 0x63, 0xc8, 0xa7, 0x13]
        );
        assert_eq!(
            pk_d,
            [
                0x04, 0x54, 0xc0, 0x14, 0x13, 0x5e, 0xc6, 0x95, 0xa1, 0x86, 0x0f, 0x8d, 0x65, 0xb3,
                0x73, 0x54, 0x6b, 0x62, 0x3f, 0x38, 0x8a, 0xbb, 0xec, 0xd0, 0xc8, 0xb2, 0x11, 0x1a,
                0xbd, 0xec, 0x30, 0x1d
            ]
        );
    }

    #[test]
    fn test_div() {
        let nk = [
            0xf7, 0xcf, 0x9e, 0x77, 0xf2, 0xe5, 0x86, 0x83, 0x38, 0x3c, 0x15, 0x19, 0xac, 0x7b,
            0x06, 0x2d, 0x30, 0x04, 0x0e, 0x27, 0xa7, 0x25, 0xfb, 0x88, 0xfb, 0x19, 0xa9, 0x78,
            0xbd, 0x3f, 0xd6, 0xba,
        ];
        let ak = [
            0xf3, 0x44, 0xec, 0x38, 0x0f, 0xe1, 0x27, 0x3e, 0x30, 0x98, 0xc2, 0x58, 0x8c, 0x5d,
            0x3a, 0x79, 0x1f, 0xd7, 0xba, 0x95, 0x80, 0x32, 0x76, 0x07, 0x77, 0xfd, 0x0e, 0xfa,
            0x8e, 0xf1, 0x16, 0x20,
        ];

        let ivk: [u8; 32] = aknk_to_ivk(&ak, &nk);
        let default_d = [
            0xf1, 0x9d, 0x9b, 0x79, 0x7e, 0x39, 0xf3, 0x37, 0x44, 0x58, 0x39,
        ];

        let result = pkd_group_hash(&default_d);
        let x = super::AffinePoint::from_bytes(result);
        if x.is_some().unwrap_u8() == 1 {
            let y = super::ExtendedPoint::from(x.unwrap());
            let v = y.to_niels().multiply_bits(&ivk);
            let t = super::AffinePoint::from(v);
            let pk_d = t.to_bytes();
            assert_eq!(
                pk_d,
                [
                    0xdb, 0x4c, 0xd2, 0xb0, 0xaa, 0xc4, 0xf7, 0xeb, 0x8c, 0xa1, 0x31, 0xf1, 0x65,
                    0x67, 0xc4, 0x45, 0xa9, 0x55, 0x51, 0x26, 0xd3, 0xc2, 0x9f, 0x14, 0xe3, 0xd7,
                    0x76, 0xe8, 0x41, 0xae, 0x74, 0x15
                ]
            );
        }
    }

    #[test]
    fn test_default_diversifier_fromlist() {
        let seed = [0u8; 32];
        let list = ff1aes_list(&seed);
        let default_d = default_diversifier_fromlist(&list);
        assert_eq!(
            default_d,
            [0xdc, 0xe7, 0x7e, 0xbc, 0xec, 0x0a, 0x26, 0xaf, 0xd6, 0x99, 0x8c]
        );
    }

    #[test]
    fn test_defaultpkd() {
        let seed = [0u8; 32];
        let default_d = default_diversifier(&seed);

        let nk = [
            0xf7, 0xcf, 0x9e, 0x77, 0xf2, 0xe5, 0x86, 0x83, 0x38, 0x3c, 0x15, 0x19, 0xac, 0x7b,
            0x06, 0x2d, 0x30, 0x04, 0x0e, 0x27, 0xa7, 0x25, 0xfb, 0x88, 0xfb, 0x19, 0xa9, 0x78,
            0xbd, 0x3f, 0xd6, 0xba,
        ];
        let ak = [
            0xf3, 0x44, 0xec, 0x38, 0x0f, 0xe1, 0x27, 0x3e, 0x30, 0x98, 0xc2, 0x58, 0x8c, 0x5d,
            0x3a, 0x79, 0x1f, 0xd7, 0xba, 0x95, 0x80, 0x32, 0x76, 0x07, 0x77, 0xfd, 0x0e, 0xfa,
            0x8e, 0xf1, 0x16, 0x20,
        ];

        let ivk: [u8; 32] = aknk_to_ivk(&ak, &nk);

        let pkd = default_pkd(&ivk, &default_d);
        assert_eq!(
            pkd,
            [
                0xdb, 0x4c, 0xd2, 0xb0, 0xaa, 0xc4, 0xf7, 0xeb, 0x8c, 0xa1, 0x31, 0xf1, 0x65, 0x67,
                0xc4, 0x45, 0xa9, 0x55, 0x51, 0x26, 0xd3, 0xc2, 0x9f, 0x14, 0xe3, 0xd7, 0x76, 0xe8,
                0x41, 0xae, 0x74, 0x15
            ]
        );
    }

    #[test]
    fn test_grouphash_default() {
        let default_d = [
            0xf1, 0x9d, 0x9b, 0x79, 0x7e, 0x39, 0xf3, 0x37, 0x44, 0x58, 0x39,
        ];

        let result = zip32::pkd_group_hash(&default_d);
        let x = super::AffinePoint::from_bytes(result);
        assert_eq!(x.is_some().unwrap_u8(), 1);
        assert_eq!(
            result,
            [
                0x3a, 0x71, 0xe3, 0x48, 0x16, 0x9e, 0x0c, 0xed, 0xbc, 0x4f, 0x36, 0x33, 0xa2, 0x60,
                0xd0, 0xe7, 0x85, 0xea, 0x8f, 0x89, 0x27, 0xce, 0x45, 0x01, 0xce, 0xf3, 0x21, 0x6e,
                0xd0, 0x75, 0xce, 0xa2
            ]
        );
    }

    #[test]
    fn test_ak() {
        let seed = [0u8; 32];
        let ask: [u8; 32] = sapling_derive_dummy_ask(&seed);
        assert_eq!(
            ask,
            [
                0x85, 0x48, 0xa1, 0x4a, 0x47, 0x3e, 0xa5, 0x47, 0xaa, 0x23, 0x78, 0x40, 0x20, 0x44,
                0xf8, 0x18, 0xcf, 0x19, 0x11, 0xcf, 0x5d, 0xd2, 0x05, 0x4f, 0x67, 0x83, 0x45, 0xf0,
                0x0d, 0x0e, 0x88, 0x06
            ]
        );
        let ak: [u8; 32] = sapling_ask_to_ak(&ask);
        assert_eq!(
            ak,
            [
                0xf3, 0x44, 0xec, 0x38, 0x0f, 0xe1, 0x27, 0x3e, 0x30, 0x98, 0xc2, 0x58, 0x8c, 0x5d,
                0x3a, 0x79, 0x1f, 0xd7, 0xba, 0x95, 0x80, 0x32, 0x76, 0x07, 0x77, 0xfd, 0x0e, 0xfa,
                0x8e, 0xf1, 0x16, 0x20
            ]
        );
    }

    #[test]
    fn test_nk() {
        let seed = [0u8; 32];

        let nsk: [u8; 32] = sapling_derive_dummy_nsk(&seed);
        assert_eq!(
            nsk,
            [
                0x30, 0x11, 0x4e, 0xa0, 0xdd, 0x0b, 0xb6, 0x1c, 0xf0, 0xea, 0xea, 0xb6, 0xec, 0x33,
                0x31, 0xf5, 0x81, 0xb0, 0x42, 0x5e, 0x27, 0x33, 0x85, 0x01, 0x26, 0x2d, 0x7e, 0xac,
                0x74, 0x5e, 0x6e, 0x05
            ]
        );

        let nk: [u8; 32] = sapling_nsk_to_nk(&nsk);
        assert_eq!(
            nk,
            [
                0xf7, 0xcf, 0x9e, 0x77, 0xf2, 0xe5, 0x86, 0x83, 0x38, 0x3c, 0x15, 0x19, 0xac, 0x7b,
                0x06, 0x2d, 0x30, 0x04, 0x0e, 0x27, 0xa7, 0x25, 0xfb, 0x88, 0xfb, 0x19, 0xa9, 0x78,
                0xbd, 0x3f, 0xd6, 0xba
            ]
        );
    }

    #[test]
    fn test_ivk() {
        let nk = [
            0xf7, 0xcf, 0x9e, 0x77, 0xf2, 0xe5, 0x86, 0x83, 0x38, 0x3c, 0x15, 0x19, 0xac, 0x7b,
            0x06, 0x2d, 0x30, 0x04, 0x0e, 0x27, 0xa7, 0x25, 0xfb, 0x88, 0xfb, 0x19, 0xa9, 0x78,
            0xbd, 0x3f, 0xd6, 0xba,
        ];
        let ak = [
            0xf3, 0x44, 0xec, 0x38, 0x0f, 0xe1, 0x27, 0x3e, 0x30, 0x98, 0xc2, 0x58, 0x8c, 0x5d,
            0x3a, 0x79, 0x1f, 0xd7, 0xba, 0x95, 0x80, 0x32, 0x76, 0x07, 0x77, 0xfd, 0x0e, 0xfa,
            0x8e, 0xf1, 0x16, 0x20,
        ];

        let ivk: [u8; 32] = aknk_to_ivk(&ak, &nk);
        assert_eq!(
            ivk,
            [
                0xb7, 0x0b, 0x7c, 0xd0, 0xed, 0x03, 0xcb, 0xdf, 0xd7, 0xad, 0xa9, 0x50, 0x2e, 0xe2,
                0x45, 0xb1, 0x3e, 0x56, 0x9d, 0x54, 0xa5, 0x71, 0x9d, 0x2d, 0xaa, 0x0f, 0x5f, 0x14,
                0x51, 0x47, 0x92, 0x04
            ]
        );
    }

    use crate::*;
    use alloc::vec::Vec;

    fn encode_test(v: &[u8]) -> Vec<u8> {
        let n = if v.len() % 8 > 0 {
            1 + v.len() / 8
        } else {
            v.len() / 8
        };
        let mut result: Vec<u8> = std::vec::Vec::new();
        let mut i = 0;
        while i < n {
            result.push(0);
            for j in 0..8 {
                let s = if i * 8 + j < v.len() { v[i * 8 + j] } else { 0 };
                result[i] += s;
                if j < 7 {
                    result[i] <<= 1;
                }
            }
            i += 1;
        }
        result
    }

    #[test]
    fn test_encode_test() {
        let f1: [u8; 9] = [0, 0, 0, 0, 0, 0, 0, 1, 1];
        assert_eq!(encode_test(&f1).as_slice(), &[1, 128]);
    }

    #[test]
    fn test_handlechunk() {
        let bits: u8 = 1;
        let mut cur = Fr::one();
        let tmp = handle_chunk(bits, &mut cur);
        //     assert_eq!(tmp.to_bytes(),[3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_key_small() {
        let m: [u8; 1] = [0xb0; 1];
        assert_eq!(
            pedersen_hash(&m, 3),
            [
                115, 27, 180, 151, 186, 120, 30, 98, 134, 221, 162, 136, 54, 82, 230, 141, 30, 114,
                188, 151, 176, 20, 4, 182, 255, 43, 30, 173, 67, 98, 64, 22
            ]
        );
    }

    #[test]
    fn test_pedersen_ledger() {
        let m: [u8; 32] = [0xb0; 32];
        let mut output = [0u8; 32];
        do_pedersen_hash(m.as_ptr(), output.as_mut_ptr());
        assert_eq!(
            output,
            [
                115, 27, 180, 151, 186, 120, 30, 98, 134, 221, 162, 136, 54, 82, 230, 141, 30, 114,
                188, 151, 176, 20, 4, 182, 255, 43, 30, 173, 67, 98, 64, 22
            ]
        );
    }

    #[test]
    fn test_pedersen_small() {
        let input_bits: [u8; 9] = [1, 1, 1, 1, 1, 1, 1, 0, 0];
        let m = encode_test(&input_bits);
        let h = pedersen_hash(&m, 9);
        assert_eq!(pedersen_hash(&[254, 0], 9), h);
    }

    #[test]
    fn test_pedersen_onechunk() {
        let input_bits: [u8; 189] = [
            1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0,
            0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0,
            0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 1,
            1, 1, 0, 0, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1,
            0, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, 0, 1,
            1, 1, 0, 1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1, 1, 0, 1, 0, 0, 0, 1, 0,
            1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0,
        ];
        let m = encode_test(&input_bits);
        let h = pedersen_hash(&m, input_bits.len() as u64);
        assert_eq!(
            h,
            [
                0xdd, 0xf5, 0x21, 0xad, 0xc3, 0xa5, 0x97, 0xf5, 0xcf, 0x72, 0x29, 0xff, 0x02, 0xcf,
                0xed, 0x7e, 0x94, 0x9f, 0x01, 0xb6, 0x1d, 0xf3, 0xe1, 0xdc, 0xdf, 0xf5, 0x20, 0x76,
                0x31, 0x10, 0xa5, 0x2d
            ]
        );
    }

    #[test]
    fn test_pedersen_big() {
        let input_bits: [u8; 190] = [
            1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 1, 1,
            0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1,
            1, 1, 1, 0, 1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1,
            1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1,
            0, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 0,
            0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1,
        ];
        let m = encode_test(&input_bits);
        let h = pedersen_hash(&m, input_bits.len() as u64);
        assert_eq!(
            h,
            [
                0x40, 0x0c, 0xf2, 0x1e, 0xeb, 0x6f, 0x8e, 0x59, 0x4a, 0x0e, 0xcd, 0x2b, 0x7f, 0x7a,
                0x68, 0x46, 0x34, 0xd9, 0x6e, 0xdf, 0x51, 0xfb, 0x3d, 0x19, 0x2d, 0x99, 0x40, 0xe6,
                0xc7, 0x47, 0x12, 0x60
            ]
        );

        let inp2: [u8; 756] = [
            1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0,
            1, 0, 1, 1, 1, 1, 0, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 1,
            1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1,
            1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 1, 0, 1, 1, 1, 0,
            1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 1, 1, 1,
            0, 0, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0, 1, 1, 0, 0, 0, 1, 0, 0,
            0, 0, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 1, 0, 0,
            1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 1, 1, 1, 0, 1, 1, 1,
            1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1,
            0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 1, 1, 0,
            0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 0,
            1, 1, 1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 1, 1,
            1, 0, 0, 1, 0, 1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 0, 1, 0, 0,
            1, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 1, 0, 1, 0, 1,
            0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
            0, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 0, 1, 0, 0,
            0, 1, 0, 0, 0, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1,
            1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0,
            0, 1, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0,
            1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1, 0, 0, 0,
            1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1,
            0, 1, 0, 1, 1, 0, 1, 1, 1, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 1, 1, 1, 1,
            1, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1, 0,
            1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0,
            0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 1,
            0, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1,
            1, 1,
        ];
        let m2 = encode_test(&inp2);
        let h2 = pedersen_hash(&m2, inp2.len() as u64);
        assert_eq!(
            h2,
            [
                0x27, 0xae, 0xf2, 0xe8, 0xeb, 0xed, 0xad, 0x19, 0x39, 0x37, 0x9f, 0x4f, 0x44, 0x7e,
                0xfb, 0xd9, 0x25, 0x5a, 0x87, 0x4c, 0x70, 0x08, 0x81, 0x6a, 0x80, 0xd8, 0xf2, 0xb1,
                0xec, 0x92, 0x41, 0x31
            ]
        );

        let inp3: [u8; 945] = [
            0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0,
            0, 1, 0, 1, 1, 1, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0,
            0, 1, 0, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0,
            1, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0,
            0, 1, 1, 0, 1, 0, 1, 0, 0, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0,
            1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1,
            1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1,
            1, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 1, 0, 1, 1, 1, 1, 0,
            0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1,
            0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 1, 0, 0, 1, 1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 1,
            0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1,
            0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1, 0, 0, 0, 1, 1, 0, 1, 1, 0, 1,
            0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 1, 0, 1, 0, 1, 0,
            0, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1,
            1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0,
            0, 1, 0, 0, 0, 1, 1, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 0,
            1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1,
            1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1,
            1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0,
            0, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 1, 1,
            1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1,
            1, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0,
            1, 0, 1, 1, 1, 0, 1, 0, 1, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1,
            1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, 1, 1, 1, 0,
            0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1,
            1, 1, 0, 1, 0, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 0, 1, 0, 0, 1, 0, 1,
            0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1, 1,
            0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0,
            0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 1,
            0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 1, 0, 0, 1, 1,
            1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1,
            0, 0, 0, 1, 0, 1, 1, 1, 1, 0, 0, 1, 0, 0, 1, 1, 0,
        ];
        let m3 = encode_test(&inp3);
        let h3 = pedersen_hash(&m3, inp3.len() as u64);
        assert_eq!(
            h3,
            [
                0x37, 0x5f, 0xdd, 0x7b, 0x29, 0xde, 0x6e, 0x22, 0x5e, 0xbb, 0x7a, 0xe4, 0x20, 0x3c,
                0xa5, 0x0e, 0xca, 0x7c, 0x9b, 0xab, 0x97, 0x1c, 0xc6, 0x91, 0x3c, 0x6f, 0x13, 0xed,
                0xf3, 0x27, 0xe8, 0x00
            ]
        );
    }
    /*
    #[test]
    fn test_sharedsecret() {
        let esk: [u8; 32] = [
            0x81, 0xc7, 0xb2, 0x17, 0x1f, 0xf4, 0x41, 0x52, 0x50, 0xca, 0xc0, 0x1f, 0x59, 0x82,
            0xfd, 0x8f, 0x49, 0x61, 0x9d, 0x61, 0xad, 0x78, 0xf6, 0x83, 0x0b, 0x3c, 0x60, 0x61,
            0x45, 0x96, 0x2a, 0x0e,
        ];
        let pk_d: [u8; 32] = [
            0x88, 0x99, 0xc6, 0x44, 0xbf, 0xc6, 0x0f, 0x87, 0x83, 0xf9, 0x2b, 0xa9, 0xf8, 0x18,
            0x9e, 0xd2, 0x77, 0xbf, 0x68, 0x3d, 0x5d, 0x1d, 0xae, 0x02, 0xc5, 0x71, 0xff, 0x47,
            0x86, 0x9a, 0x0b, 0xa6,
        ];
        let sharedsecret: [u8; 32] = [
            0x2e, 0x35, 0x7d, 0x82, 0x2e, 0x02, 0xdc, 0xe8, 0x84, 0xee, 0x94, 0x8a, 0xb4, 0xff,
            0xb3, 0x20, 0x6b, 0xa5, 0x74, 0x77, 0xac, 0x7d, 0x7b, 0x07, 0xed, 0x44, 0x6c, 0x3b,
            0xe4, 0x48, 0x1b, 0x3e,
        ];
        assert_eq!(sapling_ka_agree(esk, pk_d), sharedsecret);
    }

    #[test]
    fn test_encryption() {
        let k_enc = [
            0x6d, 0xf8, 0x5b, 0x17, 0x89, 0xb0, 0xb7, 0x8b, 0x46, 0x10, 0xf2, 0x5d, 0x36, 0x8c,
            0xb5, 0x11, 0x14, 0x0a, 0x7c, 0x0a, 0xf3, 0xbc, 0x3d, 0x2a, 0x22, 0x6f, 0x92, 0x7d,
            0xe6, 0x02, 0xa7, 0xf1,
        ];
        let p_enc = [
            0x01, 0xdc, 0xe7, 0x7e, 0xbc, 0xec, 0x0a, 0x26, 0xaf, 0xd6, 0x99, 0x8c, 0x00, 0xe1,
            0xf5, 0x05, 0x00, 0x00, 0x00, 0x00, 0x39, 0x17, 0x6d, 0xac, 0x39, 0xac, 0xe4, 0x98,
            0x0e, 0xcc, 0x8d, 0x77, 0x8e, 0x89, 0x86, 0x02, 0x55, 0xec, 0x36, 0x15, 0x06, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf6, 0x00, 0x00, 0x00,
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
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let c_enc = [
            0xbd, 0xcb, 0x94, 0x72, 0xa1, 0xac, 0xad, 0xf1, 0xd0, 0x82, 0x07, 0xf6, 0x3c, 0xaf,
            0x4f, 0x3a, 0x76, 0x3c, 0x67, 0xd0, 0x66, 0x56, 0x0a, 0xd9, 0x6c, 0x1e, 0xf9, 0x52,
            0xf8, 0x46, 0xa9, 0xc2, 0x80, 0x82, 0xdd, 0xef, 0x45, 0x21, 0xf6, 0x82, 0x54, 0x76,
            0xad, 0xe3, 0x2e, 0xeb, 0x34, 0x64, 0x06, 0xa5, 0xee, 0xc9, 0x4b, 0x4a, 0xb9, 0xe4,
            0x55, 0x12, 0x42, 0xb1, 0x44, 0xa4, 0xf8, 0xc8, 0x28, 0xbc, 0x19, 0x7f, 0x3e, 0x92,
            0x5f, 0x61, 0x7f, 0xc4, 0xb9, 0xc1, 0xb1, 0x53, 0xad, 0x15, 0x3a, 0x3c, 0x56, 0xf8,
            0x1f, 0xc4, 0x8b, 0xf5, 0x4e, 0x6e, 0xe8, 0x89, 0x5f, 0x27, 0x8c, 0x5e, 0x4c, 0x6a,
            0xe7, 0xa8, 0xa0, 0x23, 0x86, 0x70, 0x85, 0xb4, 0x07, 0xbe, 0xce, 0x40, 0x0b, 0xc6,
            0xaa, 0xec, 0x06, 0xaf, 0xf8, 0xb0, 0x49, 0xbc, 0xb2, 0x63, 0x63, 0xc6, 0xde, 0x01,
            0x8d, 0x2d, 0xa0, 0x41, 0xcc, 0x2e, 0xb8, 0xd0, 0x86, 0x4a, 0x70, 0xdf, 0x68, 0x47,
            0xb3, 0x37, 0x5a, 0x31, 0x86, 0x6c, 0x49, 0xa8, 0x02, 0x5a, 0xd7, 0x17, 0xe7, 0x79,
            0xbd, 0x0f, 0xb5, 0xce, 0xed, 0x3e, 0xc4, 0x40, 0x8e, 0x18, 0x50, 0x69, 0x4b, 0xa3,
            0x56, 0x39, 0xdd, 0x8b, 0x55, 0xd2, 0xbf, 0xdf, 0xc6, 0x40, 0x6c, 0x78, 0xc0, 0x0e,
            0xb5, 0xfc, 0x48, 0x76, 0x4b, 0xf4, 0xd8, 0x4d, 0xe1, 0xa0, 0x26, 0xd9, 0x02, 0x86,
            0x60, 0xa9, 0xa5, 0xc1, 0xc5, 0x94, 0xb8, 0x15, 0x8c, 0x69, 0x1e, 0x50, 0x68, 0xc8,
            0x51, 0xda, 0xfa, 0x30, 0x10, 0xe3, 0x9b, 0x70, 0xc4, 0x66, 0x83, 0x73, 0xbb, 0x59,
            0xac, 0x53, 0x07, 0x0c, 0x7b, 0x3f, 0x76, 0x62, 0x03, 0x84, 0x27, 0xb3, 0x72, 0xfd,
            0x75, 0x36, 0xe5, 0x4d, 0x8c, 0x8e, 0x61, 0x56, 0x2c, 0xb0, 0xe5, 0x7e, 0xf7, 0xb4,
            0x43, 0xde, 0x5e, 0x47, 0x8f, 0x4b, 0x02, 0x9c, 0x36, 0xaf, 0x71, 0x27, 0x1a, 0x0f,
            0x9d, 0x57, 0xbe, 0x80, 0x1b, 0xc4, 0xf2, 0x61, 0x8d, 0xc4, 0xf0, 0xab, 0xd1, 0x5f,
            0x0b, 0x42, 0x0c, 0x11, 0x14, 0xbb, 0xd7, 0x27, 0xe4, 0xb3, 0x1a, 0x6a, 0xaa, 0xd8,
            0xfe, 0x53, 0xb7, 0xdf, 0x60, 0xb4, 0xe0, 0xc9, 0xe9, 0x45, 0x7b, 0x89, 0x3f, 0x20,
            0xec, 0x18, 0x61, 0x1e, 0x68, 0x03, 0x05, 0xfe, 0x04, 0xba, 0x3b, 0x8d, 0x30, 0x1f,
            0x5c, 0xd8, 0x2c, 0x2c, 0x8d, 0x1c, 0x58, 0x5d, 0x51, 0x15, 0x4b, 0x46, 0x88, 0xff,
            0x5a, 0x35, 0x0b, 0x60, 0xae, 0x30, 0xda, 0x4f, 0x74, 0xc3, 0xd5, 0x5c, 0x73, 0xda,
            0xe8, 0xad, 0x9a, 0xb8, 0x0b, 0xbb, 0x5d, 0xdf, 0x1b, 0xea, 0xec, 0x12, 0x0f, 0xc4,
            0xf7, 0x8d, 0xe5, 0x4f, 0xef, 0xe1, 0xa8, 0x41, 0x35, 0x79, 0xfd, 0xce, 0xa2, 0xf6,
            0x56, 0x74, 0x10, 0x4c, 0xba, 0xac, 0x7e, 0x0d, 0xe5, 0x08, 0x3d, 0xa7, 0xb1, 0xb7,
            0xf2, 0xe9, 0x43, 0x70, 0xdd, 0x0a, 0x3e, 0xed, 0x71, 0x50, 0x36, 0x54, 0x2f, 0xa4,
            0x0e, 0xd4, 0x89, 0x2b, 0xaa, 0xfb, 0x57, 0x2e, 0xe0, 0xf9, 0x45, 0x9c, 0x1c, 0xbe,
            0x3a, 0xd1, 0xb6, 0xaa, 0xf1, 0x1f, 0x54, 0x93, 0x59, 0x52, 0xbe, 0x6b, 0x95, 0x38,
            0xa9, 0xa3, 0x9e, 0xde, 0x64, 0x2b, 0xb0, 0xcd, 0xac, 0x1c, 0x09, 0x09, 0x2c, 0xd7,
            0x11, 0x16, 0x0a, 0x8d, 0x45, 0x19, 0xb4, 0xce, 0x20, 0xff, 0xf6, 0x61, 0x2b, 0xc7,
            0xb0, 0x53, 0x93, 0xbb, 0x7e, 0x96, 0xf8, 0xea, 0x4b, 0xbc, 0x97, 0x83, 0x1f, 0x20,
            0x46, 0xe1, 0xcb, 0x5a, 0x2c, 0xe7, 0xca, 0x36, 0xfd, 0x06, 0xab, 0x39, 0x56, 0xa8,
            0x03, 0xd4, 0x32, 0x5a, 0xae, 0x72, 0xef, 0xb7, 0x07, 0xca, 0xa0, 0x44, 0xd3, 0xf8,
            0xfc, 0x7d, 0x09, 0x46, 0xbe, 0xb1, 0x1c, 0xdd, 0xc8, 0x53, 0xdb, 0xcf, 0x24, 0x3a,
            0xf3, 0xe5, 0x92, 0xb8, 0x1d, 0xb3, 0x64, 0x19, 0xd3, 0x4a, 0x4b, 0xb1, 0xee, 0x53,
            0xc1, 0xa1, 0xba, 0x51, 0xc1, 0x8b, 0x2e, 0xe9, 0x2d, 0xb4, 0xbf, 0x5f, 0xce, 0xeb,
            0x82, 0x0e, 0x8c, 0x58, 0xf8, 0x16, 0x6c, 0x3a, 0xcb, 0xf7, 0x61, 0xb5, 0xb1, 0xf2,
            0x9c, 0x3f, 0x11, 0x81, 0x67, 0xbb, 0x6c, 0xdb, 0x23, 0x30, 0x35, 0x29, 0x6a, 0xd4,
            0x0e, 0x8a, 0xa0, 0xce, 0xf5, 0x70,
        ];
        assert_eq!(chacha_encryptnote(k_enc, p_enc)[0..32], c_enc[0..32]);
        assert_eq!(chacha_decryptnote(k_enc, c_enc)[0..32], p_enc[0..32]);
    }

    #[test]
    fn test_kdf() {
        let esk: [u8; 32] = [
            0x81, 0xc7, 0xb2, 0x17, 0x1f, 0xf4, 0x41, 0x52, 0x50, 0xca, 0xc0, 0x1f, 0x59, 0x82,
            0xfd, 0x8f, 0x49, 0x61, 0x9d, 0x61, 0xad, 0x78, 0xf6, 0x83, 0x0b, 0x3c, 0x60, 0x61,
            0x45, 0x96, 0x2a, 0x0e,
        ];
        let g_d = pkd_group_hash(&[
            0xdc, 0xe7, 0x7e, 0xbc, 0xec, 0x0a, 0x26, 0xaf, 0xd6, 0x99, 0x8c,
        ]);
        let dp = derive_public(esk, g_d);

        let epk: [u8; 32] = [
            0x7e, 0xb9, 0x28, 0xf9, 0xf6, 0xd5, 0x96, 0xbf, 0xbf, 0x81, 0x4e, 0x3d, 0xd0, 0xe2,
            0x4f, 0xdc, 0x52, 0x03, 0x0f, 0xd1, 0x0f, 0x49, 0x0b, 0xa2, 0x04, 0x58, 0x68, 0xda,
            0x98, 0xf3, 0x49, 0x36,
        ];
        assert_eq!(dp, epk);
        let k_enc = [
            0x6d, 0xf8, 0x5b, 0x17, 0x89, 0xb0, 0xb7, 0x8b, 0x46, 0x10, 0xf2, 0x5d, 0x36, 0x8c,
            0xb5, 0x11, 0x14, 0x0a, 0x7c, 0x0a, 0xf3, 0xbc, 0x3d, 0x2a, 0x22, 0x6f, 0x92, 0x7d,
            0xe6, 0x02, 0xa7, 0xf1,
        ];
        let sharedsecret: [u8; 32] = [
            0x2e, 0x35, 0x7d, 0x82, 0x2e, 0x02, 0xdc, 0xe8, 0x84, 0xee, 0x94, 0x8a, 0xb4, 0xff,
            0xb3, 0x20, 0x6b, 0xa5, 0x74, 0x77, 0xac, 0x7d, 0x7b, 0x07, 0xed, 0x44, 0x6c, 0x3b,
            0xe4, 0x48, 0x1b, 0x3e,
        ];
        assert_eq!(kdf_sapling(sharedsecret, epk), k_enc);
    }

    #[test]
    fn test_ock() {
        //prf_ock(ovk, cv, cmu, ephemeral_key)
        let ovk: [u8; 32] = [
            0x98, 0xd1, 0x69, 0x13, 0xd9, 0x9b, 0x04, 0x17, 0x7c, 0xab, 0xa4, 0x4f, 0x6e, 0x4d,
            0x22, 0x4e, 0x03, 0xb5, 0xac, 0x03, 0x1d, 0x7c, 0xe4, 0x5e, 0x86, 0x51, 0x38, 0xe1,
            0xb9, 0x96, 0xd6, 0x3b,
        ];

        let cv: [u8; 32] = [
            0xa9, 0xcb, 0x0d, 0x13, 0x72, 0x32, 0xff, 0x84, 0x48, 0xd0, 0xf0, 0x78, 0xb6, 0x81,
            0x4c, 0x66, 0xcb, 0x33, 0x1b, 0x0f, 0x2d, 0x3d, 0x8a, 0x08, 0x5b, 0xed, 0xba, 0x81,
            0x5f, 0x00, 0xa8, 0xdb,
        ];

        let cmu: [u8; 32] = [
            0x8d, 0xe2, 0xc9, 0xb3, 0xf9, 0x14, 0x67, 0xd5, 0x14, 0xfe, 0x2f, 0x97, 0x42, 0x2c,
            0x4f, 0x76, 0x11, 0xa9, 0x1b, 0xb7, 0x06, 0xed, 0x5c, 0x27, 0x72, 0xd9, 0x91, 0x22,
            0xa4, 0x21, 0xe1, 0x2d,
        ];

        let epk: [u8; 32] = [
            0x7e, 0xb9, 0x28, 0xf9, 0xf6, 0xd5, 0x96, 0xbf, 0xbf, 0x81, 0x4e, 0x3d, 0xd0, 0xe2,
            0x4f, 0xdc, 0x52, 0x03, 0x0f, 0xd1, 0x0f, 0x49, 0x0b, 0xa2, 0x04, 0x58, 0x68, 0xda,
            0x98, 0xf3, 0x49, 0x36,
        ];

        let ock: [u8; 32] = [
            0x41, 0x14, 0x43, 0xfc, 0x1d, 0x92, 0x54, 0x33, 0x74, 0x15, 0xb2, 0x14, 0x7a, 0xde,
            0xcd, 0x48, 0xf3, 0x13, 0x76, 0x9c, 0x3b, 0xa1, 0x77, 0xd4, 0xcd, 0x34, 0xd6, 0xfb,
            0xd1, 0x40, 0x27, 0x0d,
        ];

        assert_eq!(prf_ock(ovk, cv, cmu, epk), ock);
    }
    */
}
