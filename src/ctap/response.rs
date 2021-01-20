// Copyright 2019-2021 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::data_formats::{
    AuthenticatorTransport, CoseKey, CredentialProtectionPolicy, PackedAttestationStatement,
    PublicKeyCredentialDescriptor, PublicKeyCredentialParameter, PublicKeyCredentialRpEntity,
    PublicKeyCredentialUserEntity,
};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use cbor::{cbor_array_vec, cbor_bool, cbor_map_btree, cbor_map_options, cbor_text};

#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug_ctap"), derive(Debug))]
pub enum ResponseData {
    AuthenticatorMakeCredential(AuthenticatorMakeCredentialResponse),
    AuthenticatorGetAssertion(AuthenticatorGetAssertionResponse),
    AuthenticatorGetNextAssertion(AuthenticatorGetAssertionResponse),
    AuthenticatorGetInfo(AuthenticatorGetInfoResponse),
    AuthenticatorClientPin(Option<AuthenticatorClientPinResponse>),
    AuthenticatorReset,
    AuthenticatorCredentialManagement(Option<AuthenticatorCredentialManagementResponse>),
    AuthenticatorSelection,
    // TODO(kaczmarczyck) dummy, extend
    AuthenticatorConfig,
    AuthenticatorVendor(AuthenticatorVendorResponse),
}

impl From<ResponseData> for Option<cbor::Value> {
    fn from(response: ResponseData) -> Self {
        match response {
            ResponseData::AuthenticatorMakeCredential(data) => Some(data.into()),
            ResponseData::AuthenticatorGetAssertion(data) => Some(data.into()),
            ResponseData::AuthenticatorGetNextAssertion(data) => Some(data.into()),
            ResponseData::AuthenticatorGetInfo(data) => Some(data.into()),
            ResponseData::AuthenticatorClientPin(data) => data.map(|d| d.into()),
            ResponseData::AuthenticatorReset => None,
            ResponseData::AuthenticatorCredentialManagement(data) => data.map(|d| d.into()),
            ResponseData::AuthenticatorSelection => None,
            ResponseData::AuthenticatorConfig => None,
            ResponseData::AuthenticatorVendor(data) => Some(data.into()),
        }
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug_ctap"), derive(Debug))]
pub struct AuthenticatorMakeCredentialResponse {
    pub fmt: String,
    pub auth_data: Vec<u8>,
    pub att_stmt: PackedAttestationStatement,
}

impl From<AuthenticatorMakeCredentialResponse> for cbor::Value {
    fn from(make_credential_response: AuthenticatorMakeCredentialResponse) -> Self {
        let AuthenticatorMakeCredentialResponse {
            fmt,
            auth_data,
            att_stmt,
        } = make_credential_response;

        cbor_map_options! {
            1 => fmt,
            2 => auth_data,
            3 => att_stmt,
        }
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug_ctap"), derive(Debug))]
pub struct AuthenticatorGetAssertionResponse {
    pub credential: Option<PublicKeyCredentialDescriptor>,
    pub auth_data: Vec<u8>,
    pub signature: Vec<u8>,
    pub user: Option<PublicKeyCredentialUserEntity>,
    pub number_of_credentials: Option<u64>,
}

impl From<AuthenticatorGetAssertionResponse> for cbor::Value {
    fn from(get_assertion_response: AuthenticatorGetAssertionResponse) -> Self {
        let AuthenticatorGetAssertionResponse {
            credential,
            auth_data,
            signature,
            user,
            number_of_credentials,
        } = get_assertion_response;

        cbor_map_options! {
            1 => credential,
            2 => auth_data,
            3 => signature,
            4 => user,
            5 => number_of_credentials,
        }
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug_ctap"), derive(Debug))]
pub struct AuthenticatorGetInfoResponse {
    pub versions: Vec<String>,
    pub extensions: Option<Vec<String>>,
    pub aaguid: [u8; 16],
    pub options: Option<BTreeMap<String, bool>>,
    pub max_msg_size: Option<u64>,
    pub pin_protocols: Option<Vec<u64>>,
    pub max_credential_count_in_list: Option<u64>,
    pub max_credential_id_length: Option<u64>,
    pub transports: Option<Vec<AuthenticatorTransport>>,
    pub algorithms: Option<Vec<PublicKeyCredentialParameter>>,
    pub default_cred_protect: Option<CredentialProtectionPolicy>,
    pub min_pin_length: u8,
    pub firmware_version: Option<u64>,
    pub max_cred_blob_length: Option<u64>,
    pub max_rp_ids_for_set_min_pin_length: Option<u64>,
    pub remaining_discoverable_credentials: Option<u64>,
}

impl From<AuthenticatorGetInfoResponse> for cbor::Value {
    fn from(get_info_response: AuthenticatorGetInfoResponse) -> Self {
        let AuthenticatorGetInfoResponse {
            versions,
            extensions,
            aaguid,
            options,
            max_msg_size,
            pin_protocols,
            max_credential_count_in_list,
            max_credential_id_length,
            transports,
            algorithms,
            default_cred_protect,
            min_pin_length,
            firmware_version,
            max_cred_blob_length,
            max_rp_ids_for_set_min_pin_length,
            remaining_discoverable_credentials,
        } = get_info_response;

        let options_cbor: Option<cbor::Value> = options.map(|options| {
            let option_map: BTreeMap<_, _> = options
                .into_iter()
                .map(|(key, value)| (cbor_text!(key), cbor_bool!(value)))
                .collect();
            cbor_map_btree!(option_map)
        });

        cbor_map_options! {
            0x01 => cbor_array_vec!(versions),
            0x02 => extensions.map(|vec| cbor_array_vec!(vec)),
            0x03 => &aaguid,
            0x04 => options_cbor,
            0x05 => max_msg_size,
            0x06 => pin_protocols.map(|vec| cbor_array_vec!(vec)),
            0x07 => max_credential_count_in_list,
            0x08 => max_credential_id_length,
            0x09 => transports.map(|vec| cbor_array_vec!(vec)),
            0x0A => algorithms.map(|vec| cbor_array_vec!(vec)),
            0x0C => default_cred_protect.map(|p| p as u64),
            0x0D => min_pin_length as u64,
            0x0E => firmware_version,
            0x0F => max_cred_blob_length,
            0x10 => max_rp_ids_for_set_min_pin_length,
            0x14 => remaining_discoverable_credentials,
        }
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug_ctap"), derive(Debug))]
pub struct AuthenticatorClientPinResponse {
    pub key_agreement: Option<CoseKey>,
    pub pin_token: Option<Vec<u8>>,
    pub retries: Option<u64>,
}

impl From<AuthenticatorClientPinResponse> for cbor::Value {
    fn from(client_pin_response: AuthenticatorClientPinResponse) -> Self {
        let AuthenticatorClientPinResponse {
            key_agreement,
            pin_token,
            retries,
        } = client_pin_response;

        cbor_map_options! {
            1 => key_agreement.map(cbor::Value::from),
            2 => pin_token,
            3 => retries,
        }
    }
}

#[derive(Default)]
#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug_ctap"), derive(Debug))]
pub struct AuthenticatorCredentialManagementResponse {
    pub existing_resident_credentials_count: Option<u64>,
    pub max_possible_remaining_resident_credentials_count: Option<u64>,
    pub rp: Option<PublicKeyCredentialRpEntity>,
    pub rp_id_hash: Option<Vec<u8>>,
    pub total_rps: Option<u64>,
    pub user: Option<PublicKeyCredentialUserEntity>,
    pub credential_id: Option<PublicKeyCredentialDescriptor>,
    pub public_key: Option<CoseKey>,
    pub total_credentials: Option<u64>,
    pub cred_protect: Option<CredentialProtectionPolicy>,
    pub large_blob_key: Option<Vec<u8>>,
}

impl From<AuthenticatorCredentialManagementResponse> for cbor::Value {
    fn from(cred_management_response: AuthenticatorCredentialManagementResponse) -> Self {
        let AuthenticatorCredentialManagementResponse {
            existing_resident_credentials_count,
            max_possible_remaining_resident_credentials_count,
            rp,
            rp_id_hash,
            total_rps,
            user,
            credential_id,
            public_key,
            total_credentials,
            cred_protect,
            large_blob_key,
        } = cred_management_response;

        cbor_map_options! {
            0x01 => existing_resident_credentials_count,
            0x02 => max_possible_remaining_resident_credentials_count,
            0x03 => rp,
            0x04 => rp_id_hash,
            0x05 => total_rps,
            0x06 => user,
            0x07 => credential_id,
            0x08 => public_key.map(cbor::Value::from),
            0x09 => total_credentials,
            0x0A => cred_protect,
            0x0B => large_blob_key,
        }
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[cfg_attr(any(test, feature = "debug_ctap"), derive(Debug))]
pub struct AuthenticatorVendorResponse {
    pub cert_programmed: bool,
    pub pkey_programmed: bool,
}

impl From<AuthenticatorVendorResponse> for cbor::Value {
    fn from(vendor_response: AuthenticatorVendorResponse) -> Self {
        let AuthenticatorVendorResponse {
            cert_programmed,
            pkey_programmed,
        } = vendor_response;

        cbor_map_options! {
            1 => cert_programmed,
            2 => pkey_programmed,
        }
    }
}

#[cfg(test)]
mod test {
    use super::super::data_formats::{PackedAttestationStatement, PublicKeyCredentialType};
    use super::super::ES256_CRED_PARAM;
    use super::*;
    use cbor::{cbor_bytes, cbor_map};
    use crypto::rng256::ThreadRng256;

    #[test]
    fn test_make_credential_into_cbor() {
        let certificate: cbor::values::KeyType = cbor_bytes![vec![0x5C, 0x5C, 0x5C, 0x5C]];
        let att_stmt = PackedAttestationStatement {
            alg: 1,
            sig: vec![0x55, 0x55, 0x55, 0x55],
            x5c: Some(vec![vec![0x5C, 0x5C, 0x5C, 0x5C]]),
            ecdaa_key_id: Some(vec![0xEC, 0xDA, 0x1D]),
        };
        let cbor_packed_attestation_statement = cbor_map! {
            "alg" => 1,
            "sig" => vec![0x55, 0x55, 0x55, 0x55],
            "x5c" => cbor_array_vec![vec![certificate]],
            "ecdaaKeyId" => vec![0xEC, 0xDA, 0x1D],
        };

        let make_credential_response = AuthenticatorMakeCredentialResponse {
            fmt: "packed".to_string(),
            auth_data: vec![0xAD],
            att_stmt,
        };
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorMakeCredential(make_credential_response).into();
        let expected_cbor = cbor_map_options! {
            1 => "packed",
            2 => vec![0xAD],
            3 => cbor_packed_attestation_statement,
        };
        assert_eq!(response_cbor, Some(expected_cbor));
    }

    #[test]
    fn test_get_assertion_into_cbor() {
        let get_assertion_response = AuthenticatorGetAssertionResponse {
            credential: None,
            auth_data: vec![0xAD],
            signature: vec![0x51],
            user: None,
            number_of_credentials: None,
        };
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorGetAssertion(get_assertion_response).into();
        let expected_cbor = cbor_map_options! {
            2 => vec![0xAD],
            3 => vec![0x51],
        };
        assert_eq!(response_cbor, Some(expected_cbor));
    }

    #[test]
    fn test_get_info_into_cbor() {
        let versions = vec!["FIDO_2_0".to_string()];
        let get_info_response = AuthenticatorGetInfoResponse {
            versions: versions.clone(),
            extensions: None,
            aaguid: [0x00; 16],
            options: None,
            max_msg_size: None,
            pin_protocols: None,
            max_credential_count_in_list: None,
            max_credential_id_length: None,
            transports: None,
            algorithms: None,
            default_cred_protect: None,
            min_pin_length: 4,
            firmware_version: None,
            max_cred_blob_length: None,
            max_rp_ids_for_set_min_pin_length: None,
            remaining_discoverable_credentials: None,
        };
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorGetInfo(get_info_response).into();
        let expected_cbor = cbor_map_options! {
            0x01 => cbor_array_vec![versions],
            0x03 => vec![0x00; 16],
            0x0D => 4,
        };
        assert_eq!(response_cbor, Some(expected_cbor));
    }

    #[test]
    fn test_get_info_optionals_into_cbor() {
        let mut options_map = BTreeMap::new();
        options_map.insert(String::from("rk"), true);
        let get_info_response = AuthenticatorGetInfoResponse {
            versions: vec!["FIDO_2_0".to_string()],
            extensions: Some(vec!["extension".to_string()]),
            aaguid: [0x00; 16],
            options: Some(options_map),
            max_msg_size: Some(1024),
            pin_protocols: Some(vec![1]),
            max_credential_count_in_list: Some(20),
            max_credential_id_length: Some(256),
            transports: Some(vec![AuthenticatorTransport::Usb]),
            algorithms: Some(vec![ES256_CRED_PARAM]),
            default_cred_protect: Some(CredentialProtectionPolicy::UserVerificationRequired),
            min_pin_length: 4,
            firmware_version: Some(0),
            max_cred_blob_length: Some(1024),
            max_rp_ids_for_set_min_pin_length: Some(8),
            remaining_discoverable_credentials: Some(150),
        };
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorGetInfo(get_info_response).into();
        let expected_cbor = cbor_map_options! {
            0x01 => cbor_array_vec![vec!["FIDO_2_0"]],
            0x02 => cbor_array_vec![vec!["extension"]],
            0x03 => vec![0x00; 16],
            0x04 => cbor_map! {"rk" => true},
            0x05 => 1024,
            0x06 => cbor_array_vec![vec![1]],
            0x07 => 20,
            0x08 => 256,
            0x09 => cbor_array_vec![vec!["usb"]],
            0x0A => cbor_array_vec![vec![ES256_CRED_PARAM]],
            0x0C => CredentialProtectionPolicy::UserVerificationRequired as u64,
            0x0D => 4,
            0x0E => 0,
            0x0F => 1024,
            0x10 => 8,
            0x14 => 150,
        };
        assert_eq!(response_cbor, Some(expected_cbor));
    }

    #[test]
    fn test_used_client_pin_into_cbor() {
        let client_pin_response = AuthenticatorClientPinResponse {
            key_agreement: None,
            pin_token: Some(vec![70]),
            retries: None,
        };
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorClientPin(Some(client_pin_response)).into();
        let expected_cbor = cbor_map_options! {
            2 => vec![70],
        };
        assert_eq!(response_cbor, Some(expected_cbor));
    }

    #[test]
    fn test_empty_client_pin_into_cbor() {
        let response_cbor: Option<cbor::Value> = ResponseData::AuthenticatorClientPin(None).into();
        assert_eq!(response_cbor, None);
    }

    #[test]
    fn test_reset_into_cbor() {
        let response_cbor: Option<cbor::Value> = ResponseData::AuthenticatorReset.into();
        assert_eq!(response_cbor, None);
    }

    #[test]
    fn test_used_credential_management_into_cbor() {
        let cred_management_response = AuthenticatorCredentialManagementResponse::default();
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorCredentialManagement(Some(cred_management_response)).into();
        let expected_cbor = cbor_map_options! {};
        assert_eq!(response_cbor, Some(expected_cbor));
    }

    #[test]
    fn test_used_credential_management_optionals_into_cbor() {
        let mut rng = ThreadRng256 {};
        let sk = crypto::ecdh::SecKey::gensk(&mut rng);
        let rp = PublicKeyCredentialRpEntity {
            rp_id: String::from("example.com"),
            rp_name: None,
            rp_icon: None,
        };
        let user = PublicKeyCredentialUserEntity {
            user_id: vec![0xFA, 0xB1, 0xA2],
            user_name: None,
            user_display_name: None,
            user_icon: None,
        };
        let cred_descriptor = PublicKeyCredentialDescriptor {
            key_type: PublicKeyCredentialType::PublicKey,
            key_id: vec![0x1D; 32],
            transports: None,
        };
        let pk = sk.genpk();
        let cose_key = CoseKey::from(pk);

        let cred_management_response = AuthenticatorCredentialManagementResponse {
            existing_resident_credentials_count: Some(100),
            max_possible_remaining_resident_credentials_count: Some(96),
            rp: Some(rp.clone()),
            rp_id_hash: Some(vec![0x1D; 32]),
            total_rps: Some(3),
            user: Some(user.clone()),
            credential_id: Some(cred_descriptor.clone()),
            public_key: Some(cose_key.clone()),
            total_credentials: Some(2),
            cred_protect: Some(CredentialProtectionPolicy::UserVerificationOptional),
            large_blob_key: Some(vec![0xBB; 64]),
        };
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorCredentialManagement(Some(cred_management_response)).into();
        let expected_cbor = cbor_map_options! {
            0x01 => 100,
            0x02 => 96,
            0x03 => rp,
            0x04 => vec![0x1D; 32],
            0x05 => 3,
            0x06 => user,
            0x07 => cred_descriptor,
            0x08 => cbor::Value::from(cose_key),
            0x09 => 2,
            0x0A => 0x01,
            0x0B => vec![0xBB; 64],
        };
        assert_eq!(response_cbor, Some(expected_cbor));
    }

    #[test]
    fn test_empty_credential_management_into_cbor() {
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorCredentialManagement(None).into();
        assert_eq!(response_cbor, None);
    }

    #[test]
    fn test_selection_into_cbor() {
        let response_cbor: Option<cbor::Value> = ResponseData::AuthenticatorSelection.into();
        assert_eq!(response_cbor, None);
    }

    #[test]
    fn test_config_into_cbor() {
        let response_cbor: Option<cbor::Value> = ResponseData::AuthenticatorConfig.into();
        assert_eq!(response_cbor, None);
    }

    #[test]
    fn test_vendor_response_into_cbor() {
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorVendor(AuthenticatorVendorResponse {
                cert_programmed: true,
                pkey_programmed: false,
            })
            .into();
        assert_eq!(
            response_cbor,
            Some(cbor_map_options! {
                1 => true,
                2 => false,
            })
        );
        let response_cbor: Option<cbor::Value> =
            ResponseData::AuthenticatorVendor(AuthenticatorVendorResponse {
                cert_programmed: false,
                pkey_programmed: true,
            })
            .into();
        assert_eq!(
            response_cbor,
            Some(cbor_map_options! {
                1 => false,
                2 => true,
            })
        );
    }
}
