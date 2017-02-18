//! Stream ciphers

use crypto::cipher::{CipherType, CipherCategory, CipherResult};
use crypto::openssl;
use crypto::table;
use crypto::CryptoMode;
use crypto::rc4_md5;
use crypto::dummy;
use crypto::crypto::CryptoCipher;

/// Basic operation of Cipher, which is a Symmetric Cipher.
///
/// The `update` method could be called multiple times, and the `finalize` method will
/// encrypt the last block
pub trait StreamCipher {
    fn update(&mut self, data: &[u8], out: &mut Vec<u8>) -> CipherResult<()>;
    fn finalize(&mut self, out: &mut Vec<u8>) -> CipherResult<()>;
}

macro_rules! define_stream_ciphers {
    ($($name:ident => $cipher:ty,)+) => {
        /// Variant cipher which contains all possible ciphers
        pub enum StreamCipherVariant {
            $(
                $name($cipher),
            )+
        }

        impl StreamCipherVariant {
            /// Creates from an actual cipher
            pub fn new<C>(cipher: C) -> StreamCipherVariant
                where StreamCipherVariant: From<C>
            {
                From::from(cipher)
            }
        }

        impl StreamCipher for StreamCipherVariant {
            fn update(&mut self, data: &[u8], out: &mut Vec<u8>) -> CipherResult<()> {
                match *self {
                    $(
                        StreamCipherVariant::$name(ref mut cipher) => cipher.update(data, out),
                    )+
                }
            }

            fn finalize(&mut self, out: &mut Vec<u8>) -> CipherResult<()> {
                match *self {
                    $(
                        StreamCipherVariant::$name(ref mut cipher) => cipher.finalize(out),
                    )+
                }
            }
        }

        $(
            impl From<$cipher> for StreamCipherVariant {
                fn from(cipher: $cipher) -> StreamCipherVariant {
                    StreamCipherVariant::$name(cipher)
                }
            }
        )+
    }
}

define_stream_ciphers! {
    TableCipher => table::TableCipher,
    DummyCipher => dummy::DummyCipher,
    Rc4Md5Cipher => rc4_md5::Rc4Md5Cipher,
    OpenSSLCipher => openssl::OpenSSLCipher,
    CryptoCipher => CryptoCipher,
}

/// Generate a specific Cipher with key and initialize vector
pub fn new_stream(t: CipherType, key: &[u8], iv: &[u8], mode: CryptoMode) -> StreamCipherVariant {
    assert!(t.category() == CipherCategory::Stream,
            "only allow initializing with stream cipher");

    match t {
        CipherType::Table => StreamCipherVariant::new(table::TableCipher::new(key, mode)),
        CipherType::Dummy => StreamCipherVariant::new(dummy::DummyCipher),

        CipherType::ChaCha20 |
        CipherType::Salsa20 => StreamCipherVariant::new(CryptoCipher::new(t, key, iv)),

        CipherType::Rc4Md5 => StreamCipherVariant::new(rc4_md5::Rc4Md5Cipher::new(key, iv, mode)),

        _ => StreamCipherVariant::new(openssl::OpenSSLCipher::new(t, key, iv, mode)),
    }
}