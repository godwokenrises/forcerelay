use crate::{core::ics24_host::identifier::ClientId, prelude::*};
use core::fmt::{Display, Error as FmtError, Formatter};
use serde_derive::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use super::error::Error;

/// Type of the client, depending on the specific consensus algorithm.
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    strum::EnumIter,
)]
pub enum ClientType {
    Tendermint = 1,
    Eth = 2,
    Ckb = 3,
    Axon = 4,
    Ckb4Ibc = 5,

    #[cfg(any(test, feature = "mocks"))]
    Mock = 255,
}

impl ClientType {
    const TENDERMINT_STR: &'static str = "07-tendermint";
    const ETH_STR: &'static str = "07-ethereum";
    const CKB_STR: &'static str = "07-ckb4eth";
    const CKB4IBC_STR: &'static str = "07-ckb4ibc";
    const AXON_STR: &'static str = "07-axon";

    #[cfg_attr(not(test), allow(dead_code))]
    const MOCK_STR: &'static str = "9999-mock";

    /// Yields the identifier of this client type as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tendermint => Self::TENDERMINT_STR,
            Self::Eth => Self::ETH_STR,
            Self::Ckb => Self::CKB_STR,
            Self::Axon => Self::AXON_STR,
            Self::Ckb4Ibc => Self::CKB4IBC_STR,

            #[cfg(any(test, feature = "mocks"))]
            Self::Mock => Self::MOCK_STR,
        }
    }
}

impl TryFrom<u64> for ClientType {
    type Error = Error;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Tendermint),
            2 => Ok(Self::Eth),
            3 => Ok(Self::Ckb),
            4 => Ok(Self::Axon),
            5 => Ok(Self::Ckb4Ibc),

            #[cfg(any(test, feature = "mocks"))]
            9999 => Ok(Self::Mock),
            _ => Err(Error::unknown_client_type(value.to_string())),
        }
    }
}

impl From<ClientId> for ClientType {
    fn from(client_id: ClientId) -> Self {
        let mut client_type = ClientType::Mock;
        for value in ClientType::iter() {
            if client_id.as_str().starts_with(value.as_str()) {
                client_type = value;
            }
        }
        client_type
    }
}

impl Display for ClientType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        write!(f, "ClientType({})", self.as_str())
    }
}

impl core::str::FromStr for ClientType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            Self::TENDERMINT_STR => Ok(Self::Tendermint),
            Self::ETH_STR => Ok(Self::Eth),
            Self::CKB_STR => Ok(Self::Ckb),
            Self::AXON_STR => Ok(Self::Axon),
            Self::CKB4IBC_STR => Ok(Self::Ckb4Ibc),

            #[cfg(any(test, feature = "mocks"))]
            Self::MOCK_STR => Ok(Self::Mock),

            _ => Err(Error::unknown_client_type(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;
    use test_log::test;

    use super::ClientType;
    use crate::core::ics02_client::error::{Error, ErrorDetail};

    #[test]
    fn parse_tendermint_client_type() {
        let client_type = ClientType::from_str("07-tendermint");

        match client_type {
            Ok(ClientType::Tendermint) => (),
            _ => panic!("parse failed"),
        }
    }

    #[test]
    fn parse_mock_client_type() {
        let client_type = ClientType::from_str("9999-mock");

        match client_type {
            Ok(ClientType::Mock) => (),
            _ => panic!("parse failed"),
        }
    }

    #[test]
    fn parse_unknown_client_type() {
        let client_type_str = "some-random-client-type";
        let result = ClientType::from_str(client_type_str);

        match result {
            Err(Error(ErrorDetail::UnknownClientType(e), _)) => {
                assert_eq!(&e.client_type, client_type_str)
            }
            _ => {
                panic!("Expected ClientType::from_str to fail with UnknownClientType, instead got",)
            }
        }
    }

    #[test]
    fn parse_mock_as_string_result() {
        let client_type = ClientType::Mock;
        let type_string = client_type.as_str();
        let client_type_from_str = ClientType::from_str(type_string).unwrap();
        assert_eq!(client_type_from_str, client_type);
    }

    #[test]
    fn parse_tendermint_as_string_result() {
        let client_type = ClientType::Tendermint;
        let type_string = client_type.as_str();
        let client_type_from_str = ClientType::from_str(type_string).unwrap();
        assert_eq!(client_type_from_str, client_type);
    }
}
