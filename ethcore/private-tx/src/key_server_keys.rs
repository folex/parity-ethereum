// Copyright 2015-2018 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Wrapper around key server responsible for access keys processing.

use std::sync::Arc;
use parking_lot::RwLock;
use ethereum_types::{H256, Address};
use ethcore::client::{BlockId, CallContract, Client, RegistryInfo};
use ethabi::FunctionOutputDecoder;

const ACL_CHECKER_CONTRACT_REGISTRY_NAME: &'static str = "secretstore_acl_checker";

use_contract!(keys_acl_contract, "res/keys_acl.json");

/// Returns the address (of the contract), that corresponds to the key
pub fn key_to_address(key: &H256) -> Address {
	Address::from_slice(&key.to_vec()[..10])
}

/// Returns the key from the key server associated with the contract
pub fn address_to_key(contract_address: &Address) -> H256 {
	// Current solution uses contract address extended with 0 as id
	let contract_address_extended: H256 = contract_address.into();

	H256::from_slice(&contract_address_extended)
}

/// Trait for keys server keys provider.
pub trait KeyProvider: Send + Sync + 'static {
	/// Account, that is used for communication with key server
	fn key_server_account(&self) -> Option<Address>;

	/// List of keys available for the account
	fn available_keys(&self, block: BlockId, account: &Address) -> Option<Vec<Address>>;

	/// Update permissioning contract
	fn update_acl_contract(&self);
}

/// Secret Store keys provider
pub struct SecretStoreKeys {
	client: Arc<Client>,
	key_server_account: Option<Address>,
	keys_acl_contract: RwLock<Option<Address>>,
}

impl SecretStoreKeys {
	/// Create provider
	pub fn new(client: Arc<Client>, key_server_account: Option<Address>) -> Self {
		SecretStoreKeys {
			client,
			key_server_account,
			keys_acl_contract: RwLock::new(None),
		}
	}

	fn keys_to_addresses(&self, keys: Option<Vec<H256>>) -> Option<Vec<Address>> {
		keys.map(|key_values| {
			let mut addresses: Vec<Address> = Vec::new();
			for key in key_values {
				addresses.push(key_to_address(&key));
			}
			addresses
		})
	}
}

impl KeyProvider for SecretStoreKeys {
	fn key_server_account(&self) -> Option<Address> {
		self.key_server_account
	}

	fn available_keys(&self, block: BlockId, account: &Address) -> Option<Vec<Address>> {
		match *self.keys_acl_contract.read() {
			Some(acl_contract_address) => {
				let (data, decoder) = keys_acl_contract::functions::available_keys::call(*account);
				if let Ok(value) = self.client.call_contract(block, acl_contract_address, data) {
					self.keys_to_addresses(decoder.decode(&value).ok())
				} else {
					None
				}
			}
			None => None,
		}
	}

	fn update_acl_contract(&self) {
		let contract_address = self.client.registry_address(ACL_CHECKER_CONTRACT_REGISTRY_NAME.into(), BlockId::Latest);
		let current_address = self.keys_acl_contract.read();

		if *current_address != contract_address {
			trace!(target: "privatetx", "Configuring for ACL checker contract from address {:?}",
				contract_address);

			let keys_acl_contract = self.keys_acl_contract.write();
			keys_acl_contract.and(contract_address);
		}
	}
}

/// Dummy keys provider.
#[derive(Default)]
pub struct StoringKeyProvider {
	available_keys: Option<Vec<Address>>,
}

impl StoringKeyProvider {
	fn set_available_keys(&mut self, keys: &Vec<Address>) {
		self.available_keys.replace(keys.to_vec());
	}
}

impl KeyProvider for StoringKeyProvider {
	fn key_server_account(&self) -> Option<Address> { None }

	fn available_keys(&self, _block: BlockId, _account: &Address) -> Option<Vec<Address>> {
		self.available_keys.clone()
	}

	fn update_acl_contract(&self) {}
}