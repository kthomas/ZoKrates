use zokrates_core::compile::{compile as core_compile, CompilationArtifacts, CompileError};

// compile(source: string, location: string): Promise<any>;
// computeWitness(artifacts: any, args: any[]): Promise<any>;
// exportVerifier(verifyingKey): Promise<string>;
// generateProof(circuit, witness, provingKey): Promise<string>;
// setup(circuit): Promise<any>;

#[no_mangle]
pub fn compile<T: Field, E: Into<imports::Error>>(
    source: String,
    location: FilePath,
    resolve_option: Option<Resolve<E>>,
) -> Result<CompilationArtifacts<T>, CompileErrors> {
    println!("Compiling {}\n", sub_matches.value_of("input").unwrap());

    let path = PathBuf::from(sub_matches.value_of("input").unwrap());

    let light = sub_matches.occurrences_of("light") > 0;

    let bin_output_path = Path::new(sub_matches.value_of("output").unwrap());

    let abi_spec_path = Path::new(sub_matches.value_of("abi_spec").unwrap());

    let hr_output_path = bin_output_path.to_path_buf().with_extension("ztf");

    let file = File::open(path.clone())
        .map_err(|why| format!("Couldn't open input file {}: {}", path.display(), why))?;

    let mut reader = BufReader::new(file);
    let mut source = String::new();
    reader.read_to_string(&mut source).unwrap();

    let fmt_error = |e: &CompileError| {
        format!(
            "{}:{}",
            e.file()
                .canonicalize()
                .unwrap()
                .strip_prefix(std::env::current_dir().unwrap())
                .unwrap()
                .display(),
            e.value()
        )
    };

    let artifacts: CompilationArtifacts<FieldPrime> =
        compile(source, path, Some(&fs_resolve)).map_err(|e| {
            format!(
                "Compilation failed:\n\n{}",
                e.0.iter()
                    .map(|e| fmt_error(e))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            )
        })?;

    let program_flattened = artifacts.prog();

    // number of constraints the flattened program will translate to.
    let num_constraints = program_flattened.constraint_count();

    // serialize flattened program and write to binary file
    let bin_output_file = File::create(&bin_output_path)
        .map_err(|why| format!("Couldn't create {}: {}", bin_output_path.display(), why))?;

    let mut writer = BufWriter::new(bin_output_file);

    serialize_into(&mut writer, &program_flattened, Infinite)
        .map_err(|_| "Unable to write data to file.".to_string())?;

    // serialize ABI spec and write to JSON file
    let abi_spec_file = File::create(&abi_spec_path)
        .map_err(|why| format!("Couldn't create {}: {}", abi_spec_path.display(), why))?;

    let abi = artifacts.abi();

    let mut writer = BufWriter::new(abi_spec_file);

    to_writer_pretty(&mut writer, &abi)
        .map_err(|_| "Unable to write data to file.".to_string())?;

    if !light {
        // write human-readable output file
        let hr_output_file = File::create(&hr_output_path).map_err(|why| {
            format!("couldn't create {}: {}", hr_output_path.display(), why)
        })?;

        let mut hrofb = BufWriter::new(hr_output_file);
        write!(&mut hrofb, "{}\n", program_flattened)
            .map_err(|_| "Unable to write data to file.".to_string())?;
        hrofb
            .flush()
            .map_err(|_| "Unable to flush buffer.".to_string())?;
    }

    if !light {
        // debugging output
        println!("Compiled program:\n{}", program_flattened);
    }

    println!("Compiled code written to '{}'", bin_output_path.display());

    if !light {
        println!("Human readable code to '{}'", hr_output_path.display());
    }

    println!("Number of constraints: {}", num_constraints);
}
