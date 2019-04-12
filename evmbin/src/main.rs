// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of Open Ethereum.

// Open Ethereum is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Open Ethereum is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Open Ethereum.  If not, see <http://www.gnu.org/licenses/>.

//! OpenEthereum EVM Interpreter Binary.
//!
//! ## Overview
//!
//! The OpenEthereum EVM interpreter binary is a tool in the OpenEthereum
//! toolchain. It is an EVM implementation for OpenEthereum that
//! is used to run a standalone version of the EVM interpreter.
//!
//! ## Usage
//!
//! The evmbin tool is not distributed with regular OpenEthereum releases
//! so you need to build it from source and run it like so:
//!
//! ```bash
//! cargo build -p evmbin --release
//! ./target/release/openethereum-evm --help
//! ```

#![warn(missing_docs)]

use std::sync::Arc;
use std::{fmt, fs};
use std::path::PathBuf;
use std::time::{Instant};

use parity_bytes::Bytes;
use bytes::ToPretty;
use docopt::Docopt;
use rustc_hex::FromHex;
use ethereum_types::{U256, Address};
use ethcore::{json_tests, test_helpers::TrieSpec};
use spec;
use serde::Deserialize;
use vm::{ActionParams, ActionType};

//mod info;
//mod display;

use crate::info::{Informant, TxInput};

const USAGE: &'static str = r#"
EVM implementation for OpenEthereum.
  Copyright 2015-2020 Parity Technologies (UK) Ltd.

Usage:
    openethereum-evm [options]
    openethereum-evm [-h | --help]

Transaction options:
    --code-file CODEFILE    Read contract code from file as hex (without 0x).
    --code CODE        Contract code as hex (without 0x).
    --to ADDRESS       Recipient address (without 0x).
    --from ADDRESS     Sender address (without 0x).
    --input DATA       Input data as hex (without 0x).
    --expected DATA    Expected return data as hex (without 0x).
    --gas GAS          Supplied gas as hex (without 0x).
    --gas-price WEI    Supplied gas price as hex (without 0x).

    -h, --help         Display this message and exit.
"#;

fn main() {
	panic_hook::set_abort();
	env_logger::init();

	let args: Args = Docopt::new(USAGE).and_then(|d| d.deserialize()).unwrap_or_else(|e| e.exit());

	run_call(args)
}

fn run_call(args: Args) {
	let _from = arg(args.from(), "--from");
	let _to = arg(args.to(), "--to");
	let code_file = arg(args.code_file(), "--code-file");
	let code = arg(args.code(), "--code");
	let _gas = arg(args.gas(), "--gas");
	let _gas_price = arg(args.gas_price(), "--gas-price");
	let calldata = arg(args.data(), "--input");
    let expected = arg(args.expected(), "--expected");

	if code.is_none() && code_file.is_none() {
		die("Either --code or --code-file is required.");
	}

    if expected.is_none() {
        die("Expected return data --expected is required.");
    }

    let code = code_file.unwrap();
    let expected_return = expected.unwrap().clone();

    //let gas = U256::from(::std::usize::MAX);
    let gas = U256::from(100000000); // 100 million startgas

    let mut params = ActionParams::default();
    params.gas = gas;
    params.code = Some(Arc::new(code.clone()));
    params.data = calldata.clone();

    let spec = ethcore::ethereum::new_constantinople_test();
    let mut test_client = ethcore::client::EvmTestClient::new(&spec).unwrap();
    let call_result = test_client.call(params, &mut ethcore::trace::NoopTracer, &mut ethcore::trace::NoopVMTracer).unwrap();
    let return_data = call_result.return_data.to_vec().to_hex();
    println!("return_data: {:?}", return_data);
    println!("gas used: {:?}", gas - call_result.gas_left);

    if return_data != expected_return {
        println!("Wrong return data!  got: {:?}   expected: {:?}", return_data, expected_return);
        die("wrong return data.");
    }


    let iterations = 100;
    let mut total_duration = std::time::Duration::new(0, 0);

    for _i in 0..iterations {
        let mut params = ActionParams::default();
        params.gas = gas;
        params.code = Some(Arc::new(code.clone()));
        params.data = calldata.clone();

        let spec = ethcore::ethereum::new_constantinople_test();
        let mut test_client = ethcore::client::EvmTestClient::new(&spec).unwrap();

        let start_run = Instant::now();

        let _result = test_client.call(params, &mut ethcore::trace::NoopTracer, &mut ethcore::trace::NoopVMTracer).unwrap();

        let run_duration = start_run.elapsed();
        total_duration = total_duration + run_duration;
    }

    let avg_duration = total_duration / iterations;
    println!("code avg run time: {:?}", avg_duration);

}




#[derive(Debug, Deserialize)]
struct Args {
    flag_code_file: Option<String>,
	flag_only: Option<String>,
	flag_from: Option<String>,
	flag_to: Option<String>,
	flag_code: Option<String>,
	flag_to: Option<String>,
	flag_from: Option<String>,
	flag_input: Option<String>,
	flag_gas: Option<String>,
	flag_gas_price: Option<String>,
	flag_input: Option<String>,
    flag_expected: Option<String>,
}

impl Args {
	// CLI option `--code CODE`
	/// Set the contract code in hex. Only send to either a contract code or a recipient address.
	pub fn code(&self) -> Result<Option<Bytes>, String> {
		match self.flag_code {
			Some(ref code) => code.from_hex().map(Some).map_err(to_string),
			None => Ok(None),
		}
	}

	// CLI option `--to ADDRESS`
	/// Set the recipient address in hex. Only send to either a contract code or a recipient address.
	pub fn to(&self) -> Result<Address, String> {
		match self.flag_to {
			Some(ref to) => to.parse().map_err(to_string),
			None => Ok(Address::zero()),
		}
	}

	// CLI option `--from ADDRESS`
	/// Set the sender address.
	pub fn from(&self) -> Result<Address, String> {
		match self.flag_from {
			Some(ref from) => from.parse().map_err(to_string),
			None => Ok(Address::zero()),
		}
	}

	// CLI option `--input DATA`
	/// Set the input data in hex.
	pub fn data(&self) -> Result<Option<Bytes>, String> {
		match self.flag_input {
			Some(ref input) => input.from_hex().map_err(to_string).map(Some),
			None => Ok(None),
		}
	}

	// CLI option `--gas GAS`
	/// Set the gas limit in units of gas. Defaults to max value to allow code to run for whatever time is required.
	pub fn gas(&self) -> Result<U256, String> {
		match self.flag_gas {
			Some(ref gas) => gas.parse().map_err(to_string),
			None => Ok(U256::from(u64::max_value())),
		}
	}

	// CLI option `--gas-price WEI`
	/// Set the gas price. Defaults to zero to allow the code to run even if an account with no balance
	/// is used, otherwise such accounts would not have sufficient funds to pay the transaction fee.
	/// Defaulting to zero also makes testing easier since it is not necessary to specify a special configuration file.
	pub fn gas_price(&self) -> Result<U256, String> {
		match self.flag_gas_price {
			Some(ref gas_price) => gas_price.parse().map_err(to_string),
			None => Ok(U256::zero()),
		}
	}

	pub fn expected(&self) -> Result<Option<String>, String> {
		match self.flag_expected {
			Some(ref expected) => expected.parse().map_err(to_string).map(Some),
			None => Ok(None),
		}
	}

    pub fn code_file(&self) -> Result<Option<Bytes>, String> {
        match self.flag_code_file {
            Some(ref filename) => {
                let code_hex = fs::read_to_string(filename).unwrap();
                println!("code_hex length: {:?}", code_hex.len());
                code_hex.from_hex().map_err(to_string).map(Some)
            },
            None => Ok(None),
        }
    }

}

fn arg<T>(v: Result<T, String>, param: &str) -> T {
	v.unwrap_or_else(|e| die(format!("Invalid {}: {}", param, e)))
}

fn to_string<T: fmt::Display>(msg: T) -> String {
	format!("{}", msg)
}

fn die<T: fmt::Display>(msg: T) -> ! {
	println!("{}", msg);
	::std::process::exit(-1)
}

#[cfg(test)]
mod tests {
	use common_types::transaction;
	use docopt::Docopt;
	use ethcore::test_helpers::TrieSpec;
	use ethjson::test_helpers::state::State;
	use serde::Deserialize;

	use super::{Args, USAGE, Address, run_call};
	use crate::{
		display::std_json::tests::informant,
		info::{self, TxInput}
	};

	#[derive(Debug, PartialEq, Deserialize)]
	pub struct SampleStateTests {
		pub add11: State,
		pub add12: State,
	}

	#[derive(Debug, PartialEq, Deserialize)]
	#[serde(rename_all = "camelCase")]
	pub struct ConstantinopleStateTests {
		pub create2call_precompiles: State,
	}

	fn run<T: AsRef<str>>(args: &[T]) -> Args {
		Docopt::new(USAGE).and_then(|d| d.argv(args.into_iter()).deserialize()).unwrap()
	}

	#[test]
	fn should_parse_all_the_options() {
		let args = run(&[
			"parity-evm",
			"--gas", "1",
			"--gas-price", "2",
			"--from", "0000000000000000000000000000000000000003",
			"--to", "0000000000000000000000000000000000000004",
			"--code", "05",
			"--input", "06",
		]);

		assert_eq!(args.gas(), Ok(1.into()));
		assert_eq!(args.gas_price(), Ok(2.into()));
		assert_eq!(args.from(), Ok(3.into()));
		assert_eq!(args.to(), Ok(4.into()));
		assert_eq!(args.code(), Ok(Some(vec![05])));
		assert_eq!(args.data(), Ok(Some(vec![06])));
	}

}
