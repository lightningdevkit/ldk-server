// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

#[cfg(genproto)]
extern crate prost_build;

#[cfg(genproto)]
use std::{env, fs, io::Write, path::Path};

#[cfg(genproto)]
const COPYRIGHT_HEADER: &str =
	"// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

";

/// To generate updated proto objects, run `RUSTFLAGS="--cfg genproto" cargo build`
fn main() {
	#[cfg(genproto)]
	generate_protos();
}

#[cfg(genproto)]
fn generate_protos() {
	prost_build::Config::new()
		.bytes(&["."])
		.type_attribute(
			".",
			"#[cfg_attr(feature = \"serde\", derive(serde::Serialize, serde::Deserialize))]",
		)
		.type_attribute(".", "#[cfg_attr(feature = \"serde\", serde(rename_all = \"snake_case\"))]")
		.compile_protos(
			&[
				"src/proto/api.proto",
				"src/proto/types.proto",
				"src/proto/events.proto",
				"src/proto/error.proto",
			],
			&["src/proto/"],
		)
		.expect("protobuf compilation failed");
	let out_dir = env::var("OUT_DIR").unwrap();
	println!("OUT_DIR: {}", &out_dir);
	for file in &["api.rs", "types.rs", "events.rs", "error.rs"] {
		let from_path = Path::new(&out_dir).join(file);
		let content = fs::read(&from_path).unwrap();
		let mut dest = fs::File::create(Path::new("src").join(file)).unwrap();
		dest.write_all(COPYRIGHT_HEADER.as_bytes()).unwrap();
		dest.write_all(&content).unwrap();
	}
}
