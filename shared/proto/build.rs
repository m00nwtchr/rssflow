use std::{env, fs, path::PathBuf};

fn main() {
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	let proto_root = PathBuf::from("proto");

	let protos: Vec<_> = fs::read_dir(&proto_root)
		.expect("Failed to read proto directory")
		.filter_map(|entry| {
			let path = entry.ok()?.path();
			if path.extension()? == "proto" {
				Some(path)
			} else {
				None
			}
		})
		.collect();

	let mut builder = tonic_build::configure()
		.type_attribute(".rssflow.websub.WebSub", "#[derive(Eq, Hash)]");

	#[cfg(debug_assertions)]
	{
		builder = builder.file_descriptor_set_path(out_dir.join("proto_descriptor.bin"));
	}

	builder
		.compile_protos(protos.as_slice(), &[proto_root])
		.unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
