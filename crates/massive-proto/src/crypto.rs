//! Envelope MAC, device token and asset digest.

use std::fmt;
use std::str::FromStr;

use hmac::{Hmac, Mac as _};
use md5::{Digest as _, Md5};
use sha1::Sha1;
use thiserror::Error;

use crate::field::Hex;

/// HMAC-SHA1 key for envelopes in both directions.
#[derive(Clone, PartialEq, Eq)]
pub struct MacKey(Vec<u8>);

impl MacKey {
    /// The literal at `0x8299F640`. The client verifies responses with it
    /// directly (`sub_824F9300`), independent of what it sent in field `0x44`.
    pub const RETAIL: &'static [u8; 32] = b"11111111111111111111111111111111";

    #[must_use]
    pub fn retail() -> Self {
        Self(Self::RETAIL.to_vec())
    }

    #[must_use]
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn sign(&self, data: &[u8]) -> Mac {
        // HMAC hashes keys longer than the block size, so every length is valid.
        let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(&self.0) else {
            unreachable!("HMAC accepts keys of any length")
        };
        mac.update(data);
        Mac(mac.finalize().into_bytes().into())
    }
}

impl fmt::Debug for MacKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacKey({} bytes)", self.0.len())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mac(pub [u8; 20]);

impl Mac {
    pub const LEN: usize = 20;
}

impl fmt::Debug for Mac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mac({})", Hex(&self.0))
    }
}

impl fmt::Display for Mac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Hex(&self.0).fmt(f)
    }
}

/// Console Ethernet address, the input of the device token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddress(pub [u8; 6]);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid hardware address {input:?}: expected six hex octets separated by ':'")]
pub struct InvalidMacAddress {
    pub input: String,
}

impl FromStr for MacAddress {
    type Err = InvalidMacAddress;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || InvalidMacAddress {
            input: s.to_owned(),
        };
        let mut out = [0u8; 6];
        let mut parts = s.split(':');
        for octet in &mut out {
            let part = parts.next().ok_or_else(err)?;
            if part.len() != 2 {
                return Err(err());
            }
            *octet = u8::from_str_radix(part, 16).map_err(|_| err())?;
        }
        if parts.next().is_some() {
            return Err(err());
        }
        Ok(Self(out))
    }
}

impl fmt::Display for MacAddress {
    /// Uppercase with colons, the form `sub_824FC9F8` hashes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, octet) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(":")?;
            }
            write!(f, "{octet:02X}")?;
        }
        Ok(())
    }
}

/// Field `0x1D`: lowercase hex of `MD5("AA:BB:CC:DD:EE:FF")`, 32 characters.
///
/// The writer uses `strnlen`, so the NUL of the 33-byte buffer is not sent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceToken(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid device token {token:?}: expected 32 lowercase hex characters")]
pub struct InvalidDeviceToken {
    pub token: String,
}

impl DeviceToken {
    pub const LEN: usize = 32;

    #[must_use]
    pub fn from_mac_address(mac: MacAddress) -> Self {
        let digest = Md5::digest(mac.to_string().as_bytes());
        Self(Hex(&digest).to_string())
    }

    pub fn parse(token: &str) -> Result<Self, InvalidDeviceToken> {
        let valid = token.len() == Self::LEN
            && token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if valid {
            Ok(Self(token.to_owned()))
        } else {
            Err(InvalidDeviceToken {
                token: token.to_owned(),
            })
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Plain SHA-1 of a downloaded asset (`sub_825056E0`), compared byte by byte
/// with the first 20 bytes of asset field `0x04`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetDigest(pub [u8; 20]);

impl AssetDigest {
    pub const LEN: usize = 20;

    #[must_use]
    pub fn of(data: &[u8]) -> Self {
        Self(Sha1::digest(data).into())
    }
}

impl fmt::Debug for AssetDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetDigest({})", Hex(&self.0))
    }
}

impl fmt::Display for AssetDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Hex(&self.0).fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_token_matches_md5_of_uppercase_address() {
        let mac: MacAddress = "00:11:22:aa:bb:cc".parse().unwrap();
        let token = DeviceToken::from_mac_address(mac);
        assert_eq!(
            token.as_str(),
            hex::encode(Md5::digest(b"00:11:22:AA:BB:CC"))
        );
        assert_eq!(DeviceToken::parse(token.as_str()).unwrap(), token);
    }

    #[test]
    fn mac_address_rejects_malformed_input() {
        for bad in [
            "",
            "00:11:22:33:44",
            "00:11:22:33:44:55:66",
            "0:11:22:33:44:55",
        ] {
            assert!(bad.parse::<MacAddress>().is_err(), "{bad}");
        }
    }

    #[test]
    fn hmac_matches_rfc2202_case_2() {
        let mac = MacKey::new(b"Jefe".to_vec()).sign(b"what do ya want for nothing?");
        assert_eq!(mac.to_string(), "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79");
    }
}
