use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
	let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

	let nuget_url = "https://www.nuget.org/api/v2/package/NetworkDirect/2.0.1";
	let package_path = out_path.join("pkg");

	if !package_path.exists() {
		let out_file = out_path.join("pkg.zip");

		// Download NuGet package.
		{
			let mut response = reqwest::blocking::get(nuget_url).expect("Failed to donwload NetworkDirect NuGet package");
			let mut file = fs::File::create(&out_file).expect("Failed to open output file");
			response.copy_to(&mut file).expect("Failed to save file");
		}

		// Extract relevant files.
		{
			let file = fs::File::open(&out_file).expect("Failed to open output file");
			let mut archive = zip::ZipArchive::new(file).unwrap();

			for i in 0..archive.len() {
				let mut entry = archive.by_index(i).unwrap();
				let out_path = match entry.enclosed_name() {
					Some(p) => {
						if !p.starts_with("include/") && !p.starts_with("lib/") {
							continue;
						}
						package_path.join(p)
					}
					None => continue,
				};

				if let Some(dir) = out_path.parent() {
					fs::create_dir_all(dir).unwrap();
				}

				let mut out_file = fs::File::create(&out_path).unwrap();
				std::io::copy(&mut entry, &mut out_file).unwrap();
			}
		}
	}

	let bindings = bindgen::Builder::default()
		.clang_arg(format!("-I{}", package_path.join("include").to_str().unwrap()))
		.header("src/bindings.h")
		.parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
		.derive_default(true)
		.blocklist_type("_?GUID")
		.blocklist_type("HRESULT")
		.blocklist_type("sockaddr")
		.blocklist_type("_?SOCKET_ADDRESS")
		.blocklist_type("_?SOCKET_ADDRESS_LIST")
		.blocklist_type("ADDRESS_FAMILY")
		.blocklist_type("IUnknown.*")
		.blocklist_type("_?OVERLAPPED.*")
		.blocklist_type("BOOL")
		.blocklist_type("CHAR")
		.blocklist_item("IID_IND.+")
		//.blocklist_type("DWORD")
		.blocklist_type("HANDLE")
		.allowlist_function("NdStartup")
		.allowlist_function("NdCleanup")
		.allowlist_function("NdFlushProviders")
		.allowlist_function("NdQueryAddressList")
		.allowlist_function("NdResolveAddress")
		.allowlist_function("NdCheckAddress")
		.allowlist_function("NdOpenAdapter")
		.allowlist_function("NdOpenV1Adapter")
		.allowlist_type("ND2_.+")
		.allowlist_var("ND2?_.+")
		.allowlist_type("IND2?.+")
		.rustified_enum("_ND2_REQUEST_TYPE")
		.generate()
		.expect("Unable to generate bindings");

	bindings
		.write_to_file(out_path.join("bindings.rs"))
		.expect("Couldn't write bindings");
}
