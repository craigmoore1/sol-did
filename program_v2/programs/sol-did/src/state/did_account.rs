use crate::errors::DidSolError;
use anchor_lang::prelude::*;
use bitflags::bitflags;
use num_derive::*;
use num_traits::*;

use crate::utils::convert_secp256k1pub_key_to_address;
use solana_program::{keccak, secp256k1_recover::secp256k1_recover};

#[account]
pub struct DidAccount {
    /// Version identifier
    pub version: u8,
    /// Bump
    pub bump: u8,
    /// Nonce, for protecting against replay attacks around secp256k1 signatures.
    pub nonce: u64,
    /// The initial authority key, automatically being added to the array of all Verification Methods.
    pub initial_verification_method: VerificationMethod,
    /// All verification methods
    pub verification_methods: Vec<VerificationMethod>,
    /// Services
    pub services: Vec<Service>,
    /// Controller (native) - did:sol:<controller>
    pub native_controllers: Vec<Pubkey>,
    /// Controller (others) - all others
    pub other_controllers: Vec<String>,
}

impl DidAccount {
    /// Accessor for all verification methods (including the initial one)
    /// Enables to pass several filters that are ANDed together.
    fn verification_methods(
        &self,
        filter_types: Option<&[VerificationMethodType]>,
        filter_flags: Option<VerificationMethodFlags>,
        filter_key: Option<&[u8]>,
        filter_alias: Option<&String>,
    ) -> Vec<&VerificationMethod> {
        std::iter::once(&self.initial_verification_method)
            .chain(self.verification_methods.iter())
            .filter(|vm| match filter_types {
                Some(filter_types) => {
                    filter_types.contains(&VerificationMethodType::from_u8(vm.method_type).unwrap())
                }
                None => true,
            })
            .filter(|vm| match filter_flags {
                Some(filter_flags) => VerificationMethodFlags::from_bits(vm.flags)
                    .unwrap()
                    .contains(filter_flags),
                None => true,
            })
            .filter(|vm| match filter_key {
                Some(filter_key) => vm.key_data == filter_key,
                None => true,
            })
            .filter(|vm| match filter_alias {
                Some(filter_alias) => vm.alias == *filter_alias,
                None => true,
            })
            .collect()
    }

    /// Accessor for all verification methods (including the initial one)
    /// Enables to pass several filters that are ANDed together.
    /// Mutable Version
    fn verification_methods_mut(
        &mut self,
        filter_types: Option<&[VerificationMethodType]>,
        filter_flags: Option<VerificationMethodFlags>,
        filter_key: Option<&[u8]>,
        filter_alias: Option<&String>,
    ) -> Vec<&mut VerificationMethod> {
        std::iter::once(&mut self.initial_verification_method)
            .chain(self.verification_methods.iter_mut())
            .filter(|vm| match filter_types {
                Some(filter_types) => {
                    filter_types.contains(&VerificationMethodType::from_u8(vm.method_type).unwrap())
                }
                None => true,
            })
            .filter(|vm| match filter_flags {
                Some(filter_flags) => VerificationMethodFlags::from_bits(vm.flags)
                    .unwrap()
                    .contains(filter_flags),
                None => true,
            })
            .filter(|vm| match filter_key {
                Some(filter_key) => vm.key_data == filter_key,
                None => true,
            })
            .filter(|vm| match filter_alias {
                Some(filter_alias) => vm.alias == *filter_alias,
                None => true,
            })
            .collect()
    }

    pub fn add_verification_method(
        &mut self,
        verification_method: VerificationMethod,
    ) -> Result<()> {
        self.verification_methods.push(verification_method);
        Ok(())
    }

    pub fn remove_verification_method(&mut self, alias: &String) -> Result<()> {
        // default case
        if alias == &self.initial_verification_method.alias {
            self.initial_verification_method.flags = 0;
            return Ok(());
        }

        // general case
        let vm_length_before = self.verification_methods.len();
        self.verification_methods.retain(|x| x.alias != *alias);
        let vm_length_after = self.verification_methods.len();

        if vm_length_after != vm_length_before {
            Ok(())
        } else {
            Err(error!(DidSolError::VmAliasNotFound))
        }
    }

    pub fn find_verification_method(&mut self, alias: &String) -> Option<&mut VerificationMethod> {
        self.verification_methods_mut(None, None, None, Some(alias))
            .into_iter()
            .next()
    }

    pub fn has_authority_verification_methods(&self) -> bool {
        !self
            .verification_methods(
                Some(&VerificationMethodType::authority_types()),
                Some(VerificationMethodFlags::CAPABILITY_INVOCATION),
                None,
                None,
            )
            .is_empty()
    }

    // TODO change to ref
    pub fn find_authority(
        &self,
        sol_authority: &Pubkey,
        eth_message: &[u8],
        eth_raw_signature: Option<&Secp256k1RawSignature>,
        filter_alias: Option<&String>,
    ) -> Option<&VerificationMethod> {
        let mut vm = self.find_sol_authority(sol_authority, filter_alias);
        if vm.is_some() {
            return vm;
        }
        vm = self.find_eth_authority(eth_message, eth_raw_signature, filter_alias);
        if vm.is_some() {
            return vm;
        }

        None
    }

    pub fn find_sol_authority(
        &self,
        authority: &Pubkey,
        filter_alias: Option<&String>,
    ) -> Option<&VerificationMethod> {
        msg!(
            "Checking if {} is an Ed25519VerificationKey2018 authority",
            authority.to_string()
        );
        self.verification_methods(
            Some(&[VerificationMethodType::Ed25519VerificationKey2018]), // TODO: is this the best way to pass this?
            Some(VerificationMethodFlags::CAPABILITY_INVOCATION),
            Some(&authority.to_bytes()),
            filter_alias,
        )
        .into_iter()
        .next()
    }

    pub fn find_eth_authority(
        &self,
        message: &[u8],
        raw_signature: Option<&Secp256k1RawSignature>,
        filter_alias: Option<&String>,
    ) -> Option<&VerificationMethod> {
        let raw_signature = raw_signature?;
        let message_with_nonce = [message, self.nonce.to_le_bytes().as_ref()].concat();
        // Ethereum conforming Message Input
        // https://docs.ethers.io/v4/api-utils.html?highlight=hashmessage#hash-function-helpers
        let sign_message_input = [
            "\x19Ethereum Signed Message:\n".as_bytes(),
            message_with_nonce.len().to_string().as_bytes(),
            message_with_nonce.as_ref(),
        ]
        .concat();

        let hash = keccak::hash(sign_message_input.as_ref());
        // msg!("Hash: {:x?}", hash.as_ref());
        // msg!("Message: {:x?}", message);
        // msg!(
        //     "sign_message_input: {:x?}, Length: {}",
        //     sign_message_input,
        //     sign_message_input.len()
        // );
        // msg!("Signature: {:x?}", raw_signature.signature);
        // msg!("RecoveryId: {:x}", raw_signature.recovery_id);

        let secp256k1_pubkey = secp256k1_recover(
            hash.as_ref(),
            raw_signature.recovery_id,
            raw_signature.signature.as_ref(),
        )
        .unwrap();
        // msg!("Recovered: {:?}", secp256k1_pubkey.to_bytes());
        //
        // // Check EcdsaSecp256k1VerificationKey2019 matches
        // msg!(
        //     "Checking if {:x?} is an EcdsaSecp256k1VerificationKey2019 authority",
        //     secp256k1_pubkey.to_bytes()
        // );
        let mut vm = self
            .verification_methods(
                Some(&[VerificationMethodType::EcdsaSecp256k1VerificationKey2019]),
                Some(VerificationMethodFlags::CAPABILITY_INVOCATION),
                Some(&secp256k1_pubkey.to_bytes()),
                filter_alias,
            )
            .into_iter()
            .next();
        if vm.is_some() {
            return vm;
        }

        let address = convert_secp256k1pub_key_to_address(&secp256k1_pubkey);
        // msg!("Address: {:?}", address);
        // // Check EcdsaSecp256k1VerificationKey2019 matches
        // msg!(
        //     "Checking if {:x?} is an EcdsaSecp256k1RecoveryMethod2020 authority",
        //     address
        // );
        vm = self
            .verification_methods(
                Some(&[VerificationMethodType::EcdsaSecp256k1RecoveryMethod2020]),
                Some(VerificationMethodFlags::CAPABILITY_INVOCATION),
                Some(&address),
                filter_alias,
            )
            .into_iter()
            .next();
        if vm.is_some() {
            return vm;
        }

        None
    }

    pub fn size(&self) -> usize {
        1 // version
        + 1 // bump
        + 8 // nonce
        + VerificationMethod::default_size() // initial_authority
        + 4 + self.verification_methods.iter().fold(0, | accum, item| { accum + item.size() }) // verification_methods
        + 4 + self.services.iter().fold(0, | accum, item| { accum + item.size() }) // services
        + 4 + self.native_controllers.len() * 32 // native_controllers
        + 4 + self.other_controllers.iter().fold(0, | accum, item| { accum + 4 + item.len() })
        // other_controllers
    }

    pub fn initial_size() -> usize {
        1 // version
        + 1 // bump
        + 8 // nonce
        + VerificationMethod::default_size() // initial_authority
        + 4 // verification_methods
        + 4 // services
        + 4 // native_controllers
        + 4 // other_controllers
    }
}

#[derive(
    Debug, AnchorSerialize, AnchorDeserialize, Copy, Clone, FromPrimitive, ToPrimitive, PartialEq,
)]
pub enum VerificationMethodType {
    /// The main Ed25519Verification Method.
    /// https://w3c-ccg.github.io/lds-ed25519-2018/
    Ed25519VerificationKey2018,
    /// Verification Method for For 20-bytes Ethereum Keys
    EcdsaSecp256k1RecoveryMethod2020,
    /// Verification Method for a full 32 bytes Secp256k1 Verification Key
    EcdsaSecp256k1VerificationKey2019,
}

impl VerificationMethodType {
    pub fn authority_types() -> [VerificationMethodType; 3] {
        [
            VerificationMethodType::Ed25519VerificationKey2018,
            VerificationMethodType::EcdsaSecp256k1VerificationKey2019,
            VerificationMethodType::EcdsaSecp256k1RecoveryMethod2020,
        ]
    }

    // pub fn is_authority_type(vm_type: u8) -> bool {
    //     let vm_type = VerificationMethodType::from_u8(vm_type).unwrap();
    //     VerificationMethodType::authority_types().contains(&vm_type)
    // }
}

impl Default for VerificationMethodType {
    fn default() -> Self {
        VerificationMethodType::Ed25519VerificationKey2018
    }
}

/// The native authority key for a [`DidAccount`]
#[derive(Debug, AnchorSerialize, AnchorDeserialize, Clone)]
pub struct VerificationMethod {
    /// alias
    pub alias: String,
    /// The permissions this key has
    pub flags: u16,
    /// The actual verification method
    pub method_type: u8, // Type: VerificationMethodType- Anchor does not yet provide mappings for enums
    /// Dynamically sized key matching the given VerificationType
    pub key_data: Vec<u8>,
}

impl VerificationMethod {
    pub fn size(&self) -> usize {
        4 + self.alias.len()
        + 2 // flags
        + 1 // method
        + 4 + self.key_data.len()
    }

    pub fn default(flags: VerificationMethodFlags, key_data: Vec<u8>) -> VerificationMethod {
        VerificationMethod {
            alias: String::from("default"),
            flags: flags.bits(),
            method_type: VerificationMethodType::default().to_u8().unwrap(),
            key_data,
        }
    }

    pub fn default_size() -> usize {
        4 + 7 // alias "default"
        + 2 // flags
        + 1 // method
        + 4 + 32 // ed25519 pubkey
    }
}

/// A Service Definition [`DidAccount`]
#[derive(Debug, AnchorSerialize, AnchorDeserialize, Default, Clone)]
pub struct Service {
    pub id: String,
    pub service_type: String,
    pub service_endpoint: String,
}

impl Service {
    pub fn size(&self) -> usize {
        4 + self.id.len() + 4 + self.service_type.len() + 4 + self.service_endpoint.len()
    }
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct Secp256k1RawSignature {
    signature: [u8; 64],
    recovery_id: u8,
}

bitflags! {
    pub struct VerificationMethodFlags: u16 {
        /// The VM is able to authenticate the subject
        const AUTHENTICATION = 1 << 0;
        /// The VM is able to proof assertions on the subject
        const ASSERTION = 1 << 1;
        /// The VM can be used for encryption
        const KEY_AGREEMENT = 1 << 2;
        /// The VM can be used for issuing capabilities. Required for DID Update
        const CAPABILITY_INVOCATION = 1 << 3;
        /// The VM can be used for delegating capabilities.
        const CAPABILITY_DELEGATION = 1 << 4;
        /// The VM is hidden from the DID Document (off-chain only)
        const DID_DOC_HIDDEN = 1 << 5;
        /// The subject did proof to be in possession of the private key
        const OWNERSHIP_PROOF = 1 << 6;
    }
}
