//! # AES 256 GCM with Enhancement
//! 
//! Inspired by simplicity of (let's say JWT) this crate provide high-level
//! abstraction for AES-GCM Crate with some enhancement such as expiration check
//! 
//! > **notes** : if you want to use os environment for "SINGLE" key management
//! > you can use `AES_GCM_SECRET` env key
//! 
//! `WARNING : IN PRODUCTION DO NOT LEAVE SECRET EMPTY, AVOID AT ANY COSTS`
//! 
//! `WARNING : THIS CRATE DOES NOT IMPLEMENT "SIV"`
//! 
//! Content:
//! - Allowed Encrypted Data
//! - Returned Data
//! - Initialization
//! - Options
//! - Decrypting
//! - Error Coverage
//! - Usage Recommendation
//! 
//! ---
//! 
//! #### Allowed data consumption
//! 
//! any data that can implement serde::Serialize
//! 
//! #### Returned data
//! 
//! any data that can implement serde::Deserialize
//! 
//! 
//! ---
//! #### Initialization :: Using OS ENV
//! ```rust
//! use aes_256_gcm::Client;
//! 
//! fn main(){
//!     // this will consume AES_GCM_SECRET
//!     // if no env found it would set to default empty
//!     let aes_client = Client::new(None);
//! }
//! ```
//! 
//! #### Initialization :: Using Custom Secret
//! ```rust
//! use aes_256_gcm::Client;
//! 
//! fn main(){
//!     let my_secret:&str = "some of my secret";
//!     let aes_client = Client::new(my_secret);
//! }
//! ```
//! 
//! #### Options :: Normal without expiration
//! ```rust
//! let result: Result<String, AesError> = client.encrypt(
//!     "something or struct", None
//! );
//! ```
//! 
//! #### Options :: With Expiration "Second"
//! ```rust
//! let result: Result<String, AesError> = client.encrypt(
//!     "something or struct",
//!     AesOptions::with_expire_second(3)
//!         .build()
//! );
//! ```
//! 
//! #### Options :: With Expiration "chrono Date"
//! ```rust
//! // let's just pretend this few days ahead
//! let date_expired = chrono::Utc::now();
//! // use it with 'with_expire_date'
//! let result: Result<String, AesError> = client.encrypt(
//!     "something or struct",
//!     AesOptions::with_expire_date(date_expired)
//!         .build()
//! );
//! ```
//! 
//! #### Decrypting
//! 
//! `YourDataType` can be `String` or `struct` or anything,\
//! make sure it is the same data type with encryption, \
//! otherwise it wont decrypting.
//! 
//! > side notes : don't forget to check their expiration :D
//! 
//! ```rust
//! 
//! let result: Result<YourDataType, AesError> = client.decrypt::<YourDataType>(&token);
//! 
//! ```
//! 
//! #### Usage Recommendation
//! 
//! for usage it's better to know well about `Arc` or around `Mutex`, 
//! and how data being shared across thread. If your application shared with multiple 
//! thread or async it's better to do best-practice of data sharing.
//! 
//! if you're integrating to some kind of framework, try to understand how their data sharing works.
//! 
//! > for other example please visit [example](https://github.com/zonblade/aes-256-gcm-rs/example)
//! > i'm opening to anyone want to contribute either example or crate improvement.
use aes_gcm::{
    aead::{generic_array::GenericArray, Aead, KeyInit, OsRng},
    aes::{
        cipher::typenum::{
            bit::{B0, B1},
            UInt, UTerm,
        },
        Aes256,
    },
    AeadCore, Aes256Gcm, AesGcm as AG,
};

type AesGcm = AG<
    Aes256,
    UInt<UInt<UInt<UInt<UTerm, B1>, B1>, B0>, B0>,
    UInt<UInt<UInt<UInt<UInt<UTerm, B1>, B0>, B0>, B0>, B0>,
>;
type AesGeneric = GenericArray<u8, UInt<UInt<UInt<UInt<UTerm, B1>, B1>, B0>, B0>>;
type AesClient = AesGcm;

#[derive(Debug, PartialEq, PartialOrd, Eq)]
pub enum AesErrorCode {
    EncryptDataNotValid,
    EncryptOptionError,
    EncryptFailed,

    DecryptDataNotValid,
    DecryptStringConvention,

    Expired,
}

#[derive(Debug)]
pub struct AesError {
    pub code: AesErrorCode,
    pub note: &'static str,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AesOptions {
    expire: Option<String>,
}

impl AesOptions {
    pub fn with_expire_second(expire: i64) -> Self {
        let microsecond = expire * 1_000_000;
        let expire = chrono::Utc::now() + chrono::Duration::microseconds(microsecond);
        let expire = expire.to_rfc3339();
        Self {
            expire: Some(expire),
        }
    }

    pub fn with_expire_date(expire: chrono::DateTime<chrono::Utc>) -> Self {
        let expire = expire.to_rfc3339();
        Self {
            expire: Some(expire),
        }
    }

    pub fn build(self) -> AesOptions {
        self
    }
}

#[derive(Clone)]
pub struct Client {
    client: AesClient,
}

impl Client {
    pub fn new(secret: impl Into<Option<&'static str>>) -> Self {
        let mut aes_secret = std::env::var("AES_GCM_SECRET").unwrap_or_else(|_| "".to_string());
        if let Some(key) = secret.into() {
            aes_secret = key.to_string();
        }

        let mut aes_secret = aes_secret.as_bytes().to_vec();
        if aes_secret.len() > 32 {
            aes_secret.truncate(32);
        } else {
            while aes_secret.len() < 32 {
                aes_secret.push(0);
            }
        }
        let aes_key = GenericArray::from_slice(&aes_secret);
        let client: AesClient = Aes256Gcm::new(&aes_key);
        Self { client }
    }

    pub fn encrypt<T>(
        &self,
        data: T,
        option: impl Into<Option<AesOptions>>,
    ) -> Result<String, AesError>
    where
        T: serde::Serialize,
    {
        let data = match serde_json::to_string(&data) {
            Ok(data) => data,
            Err(_) => return Err(AesError {
                code: AesErrorCode::EncryptDataNotValid,
                note:
                    "Data of encryption is not valid data, data must be able to serialize to json",
            }),
        };
        let mut opt = String::new();
        let optx: Option<AesOptions> = option.into();
        if let Some(optn) = optx {
            let json_opt = match serde_json::to_string(&optn) {
                Ok(data) => data,
                Err(_) => {
                    return Err(AesError {
                        code: AesErrorCode::EncryptOptionError,
                        note: "Option failed to build, please check correct parameter",
                    })
                }
            };
            opt = json_opt;
        }
        let data = format!("{}::{}", data, opt);
        let data = data.as_bytes();
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let encrypt = match self.client.encrypt(&nonce, data.as_ref()) {
            Ok(data) => data,
            Err(_) => {
                return Err(AesError {
                    code: AesErrorCode::EncryptFailed,
                    note: "Failed to encrypt data, please check correct parameter",
                })
            }
        };
        let nonce = nonce
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let data = encrypt
            .iter()
            .map(|x| format!("{:02x}", x))
            .collect::<String>();
        let data = format!("{}{}", data, nonce);
        Ok(data)
    }

    pub fn decrypt<'a, T>(&self, data: &'a str) -> Result<T, AesError>
    where
        for<'de> T: serde::Deserialize<'de>,
    {
        let (data, nonce) = data.split_at(data.len() - 24);
        let nonce = nonce
            .chars()
            .collect::<Vec<char>>()
            .chunks(2)
            .map(|x| x.iter().collect::<String>())
            .map(|x| u8::from_str_radix(&x, 16).unwrap_or(0))
            .collect::<AesGeneric>();
        let data = data
            .chars()
            .collect::<Vec<char>>()
            .chunks(2)
            .map(|x| x.iter().collect::<String>())
            .map(|x| u8::from_str_radix(&x, 16).unwrap_or(0))
            .collect::<Vec<u8>>();
        let decrypt = match self.client.decrypt(&nonce, data.as_ref()) {
            Ok(data) => data,
            Err(_) => {
                return Err(AesError {
                    code: AesErrorCode::DecryptDataNotValid,
                    note: "Input data or token does not have valid encryption data",
                })
            }
        };
        // to string
        let str_decrypt = match std::str::from_utf8(&decrypt) {
            Ok(data) => data,
            Err(_) => {
                return Err(AesError {
                    code: AesErrorCode::DecryptStringConvention,
                    note:
                        "String convention failed, either it is not valid string or not utf8 string",
                })
            }
        };
        let str_decrypt: Vec<&str> = str_decrypt.split("::").collect();
        let decrypt = str_decrypt[0];
        // decrypt to byte
        let decrypt = decrypt.as_bytes();

        if str_decrypt.len() > 1 {
            let option_decrypt = str_decrypt[1].as_bytes();
            let data_expiry = match serde_json::from_slice::<AesOptions>(&option_decrypt) {
                Ok(data) => data,
                Err(_) => AesOptions { expire: None },
            };

            if let Some(expire) = data_expiry.expire {
                let date_now = chrono::Utc::now();
                let date_exp = chrono::DateTime::parse_from_rfc3339(&expire);
                match date_exp {
                    Ok(date_exp) => {
                        if date_now > date_exp {
                            return Err(AesError {
                                code: AesErrorCode::Expired,
                                note: "This data/token simply expired",
                            });
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        let data = match serde_json::from_slice::<T>(&decrypt) {
            Ok(data) => data,
            Err(_) => {
                return Err(AesError {
                    code: AesErrorCode::DecryptDataNotValid,
                    note: "Data of decryption is not valid encrypted data",
                })
            }
        };
        Ok(data)
    }
}

#[allow(dead_code)]
fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    ///
    /// Test case 1 \
    /// Simple encrypt and decrypt string
    /// 
    #[test]
    fn test_case_1() {
        let client = Client::new(None);
        let encrypted = client.encrypt("my thing", None);
        let decrypted: String = client.decrypt(&encrypted.unwrap()).unwrap();
        assert_eq!(decrypted, "my thing");
    }

    ///
    /// Test case 2 \
    /// Simple encrypt and decrypt struct
    /// 
    #[test]
    fn test_case_2() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct TestCase2 {
            pub name: String,
        }

        let client = Client::new(None);
        let data = TestCase2 {
            name: "my name".to_string(),
        };

        let encrypted = client.encrypt(&data, None);
        let decrypted: TestCase2 = client.decrypt(&encrypted.unwrap()).unwrap();
        assert_eq!(decrypted, data);
    }

    ///
    /// Test case 3 \
    /// Simple encrypt and decrypt string with expire
    /// 
    #[test]
    fn test_case_3() {
        let client = Client::new(None);
        let encrypted = client.encrypt("my thing", AesOptions::with_expire_second(3).build());
        // sleep 5 second
        std::thread::sleep(std::time::Duration::from_secs(2));
        let decrypted = client.decrypt::<String>(&encrypted.unwrap());
        if let Ok(data) = decrypted {
            assert_eq!(data, "my thing");
        } else {
            assert!(false);
        }
    }

    ///
    /// Test case 4 \
    /// Simple encrypt and decrypt string with expire
    /// 
    #[test]
    fn test_case_4() {
        let client = Client::new(None);
        let encrypted = client.encrypt("my thing", AesOptions::with_expire_second(3).build());
        // sleep 5 second
        std::thread::sleep(std::time::Duration::from_secs(4));
        let decrypted = client.decrypt::<String>(&encrypted.unwrap());
        if let Err(e) = decrypted {
            assert_eq!(e.code, AesErrorCode::Expired);
        } else {
            assert!(false);
        }
    }

    ///
    /// Test case 5 \
    /// Simple encrypt and decrypt struct with expire
    /// 
    #[test]
    fn test_case_5() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct TestCase2 {
            pub name: String,
        }

        let client = Client::new(None);
        let data = TestCase2 {
            name: "my name".to_string(),
        };
        let encrypted = client.encrypt(data, AesOptions::with_expire_second(3).build());
        // sleep 5 second
        std::thread::sleep(std::time::Duration::from_secs(2));
        let decrypted = client.decrypt::<TestCase2>(&encrypted.unwrap());
        if let Ok(data) = decrypted {
            assert_eq!(
                data,
                TestCase2 {
                    name: "my name".to_string()
                }
            );
        } else {
            assert!(false);
        }
    }

    ///
    /// Test case 6 \
    /// Simple encrypt and decrypt struct with expire
    /// 
    #[test]
    fn test_case_6() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct TestCase2 {
            pub name: String,
        }

        let client = Client::new(None);
        let data = TestCase2 {
            name: "my name".to_string(),
        };
        let encrypted = client.encrypt(data, AesOptions::with_expire_second(3).build());
        // sleep 5 second
        std::thread::sleep(std::time::Duration::from_secs(4));
        let decrypted = client.decrypt::<TestCase2>(&encrypted.unwrap());
        if let Err(e) = decrypted {
            assert_eq!(e.code, AesErrorCode::Expired);
        } else {
            assert!(false);
        }
    }
}