
use bincode::{deserialize_from, serialize_into, Infinite};
use clap::{App, AppSettings, Arg, SubCommand};
use serde_json::{from_reader, to_writer_pretty, Value};
use std::env;
use std::fs::File;
use std::io::{stdin, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::string::String;
use zokrates_abi::Encode;
use zokrates_core::compile::{compile, CompilationArtifacts, CompileError, CompileErrors, Resolve};
use zokrates_core::imports;
use zokrates_core::ir;
use zokrates_core::proof_system::*;
use zokrates_core::typed_absy::abi::Abi;
use zokrates_core::typed_absy::{types::Signature, Type};
use zokrates_field::field::{Field, FieldPrime};
use zokrates_fs_resolver::resolve as fs_resolve;


#[no_mangle]
pub fn bl_compile<T: Field, E: Into<imports::Error>>(
    source: String,
    resolve_option: Option<Resolve<E>>,
) -> Result<CompilationArtifacts<T>, CompileErrors> {

    let path = PathBuf::from(String::from("./tmpfile"));
    let light = false;
    let bin_output_path = Path::new(String::from("./output"));
    let abi_spec_path = Path::new(String::from("./abi_spec"));
    let hr_output_path = String::from("./tmpfile.ztf");

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

#[no_mangle]
pub fn bl_computeWitness<T: Field, E: Into<imports::Error>>(
    // FIXME add params
    // artifacts: any, args: any[]
) -> Result { // FIXME add retval
    println!("Computing witness...");

    // read compiled program
    let path = Path::new(sub_matches.value_of("input").unwrap());
    let file = File::open(&path)
        .map_err(|why| format!("couldn't open {}: {}", path.display(), why))?;

    let mut reader = BufReader::new(file);

    let ir_prog: ir::Prog<FieldPrime> =
        deserialize_from(&mut reader, Infinite).map_err(|why| why.to_string())?;

    // print deserialized flattened program
    if !sub_matches.is_present("light") {
        println!("{}", ir_prog);
    }

    let is_stdin = sub_matches.is_present("stdin");
    let is_abi = sub_matches.is_present("abi");

    if !is_stdin && is_abi {
        return Err(
            "ABI input as inline argument is not supported. Please use `--stdin`.".into(),
        );
    }

    let signature = match is_abi {
        true => {
            let path = Path::new(sub_matches.value_of("abi_spec").unwrap());
            let file = File::open(&path)
                .map_err(|why| format!("couldn't open {}: {}", path.display(), why))?;
            let mut reader = BufReader::new(file);

            let abi: Abi = from_reader(&mut reader).map_err(|why| why.to_string())?;

            abi.signature()
        }
        false => Signature::new()
            .inputs(vec![Type::FieldElement; ir_prog.main.arguments.len()])
            .outputs(vec![Type::FieldElement; ir_prog.main.returns.len()]),
    };

    use zokrates_abi::Inputs;

    // get arguments
    let arguments = match is_stdin {
        // take inline arguments
        false => {
            let arguments = sub_matches.values_of("arguments");
            arguments
                .map(|a| {
                    a.map(|x| FieldPrime::try_from_dec_str(x).map_err(|_| x.to_string()))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap_or(Ok(vec![]))
                .map(|v| Inputs::Raw(v))
        }
        // take stdin arguments
        true => {
            let mut stdin = stdin();
            let mut input = String::new();

            match is_abi {
                true => match stdin.read_to_string(&mut input) {
                    Ok(_) => {
                        use zokrates_abi::parse_strict;

                        parse_strict(&input, signature.inputs)
                            .map(|parsed| Inputs::Abi(parsed))
                            .map_err(|why| why.to_string())
                    }
                    Err(_) => Err(String::from("???")),
                },
                false => match ir_prog.arguments_count() {
                    0 => Ok(Inputs::Raw(vec![])),
                    _ => match stdin.read_to_string(&mut input) {
                        Ok(_) => {
                            input.retain(|x| x != '\n');
                            input
                                .split(" ")
                                .map(|x| {
                                    FieldPrime::try_from_dec_str(x)
                                        .map_err(|_| x.to_string())
                                })
                                .collect::<Result<Vec<_>, _>>()
                                .map(|v| Inputs::Raw(v))
                        }
                        Err(_) => Err(String::from("???")),
                    },
                },
            }
        }
    }
    .map_err(|e| format!("Could not parse argument: {}", e))?;

    let interpreter = ir::Interpreter::default();

    let witness = interpreter
        .execute(&ir_prog, &arguments.encode())
        .map_err(|e| format!("Execution failed: {}", e))?;

    use zokrates_abi::Decode;

    let results_json_value: serde_json::Value =
        zokrates_abi::CheckedValues::decode(witness.return_values(), signature.outputs)
            .into();

    println!("\nWitness: \n\n{}", results_json_value);

    // write witness to file
    let output_path = Path::new(sub_matches.value_of("output").unwrap());
    let output_file = File::create(&output_path)
        .map_err(|why| format!("couldn't create {}: {}", output_path.display(), why))?;

    let writer = BufWriter::new(output_file);

    witness
        .write(writer)
        .map_err(|why| format!("could not save witness: {:?}", why))?;
}

#[no_mangle]
pub fn bl_exportVerifier<T: Field, E: Into<imports::Error>>(
    // FIXME-- add params
    // verifyingKey
) -> Result { // FIXME add retval
    let scheme = get_scheme(sub_matches.value_of("proving-scheme").unwrap())?;

    let is_abiv2 = sub_matches.value_of("solidity-abi").unwrap() == "v2";
    println!("Exporting verifier...");
    
    // read vk file
    let input_path = Path::new(sub_matches.value_of("input").unwrap());
    let input_file = File::open(&input_path)
        .map_err(|why| format!("couldn't open {}: {}", input_path.display(), why))?;
    let mut reader = BufReader::new(input_file);
    
    let mut vk = String::new();
    reader
        .read_to_string(&mut vk)
        .map_err(|why| format!("couldn't read {}: {}", input_path.display(), why))?;
    
    let verifier = scheme.export_solidity_verifier(vk, is_abiv2);
    
    //write output file
    let output_path = Path::new(sub_matches.value_of("output").unwrap());
    let output_file = File::create(&output_path)
        .map_err(|why| format!("couldn't create {}: {}", output_path.display(), why))?;
    
    let mut writer = BufWriter::new(output_file);
    
    writer
        .write_all(&verifier.as_bytes())
        .map_err(|_| "Failed writing output to file.".to_string())?;
    println!("Finished exporting verifier.");
}

#[no_mangle]
pub fn bl_generateProof<T: Field, E: Into<imports::Error>>(
    // FIXME add params
    // circuit, witness, provingKey
) -> Result { // FIXME add retval
    // generateProof(): Promise<string>;
    println!("Generating proof...");

    let scheme = get_scheme(sub_matches.value_of("proving-scheme").unwrap())?;

    // deserialize witness
    let witness_path = Path::new(sub_matches.value_of("witness").unwrap());
    let witness_file = match File::open(&witness_path) {
        Ok(file) => file,
        Err(why) => panic!("couldn't open {}: {}", witness_path.display(), why),
    };

    let witness = ir::Witness::read(witness_file)
        .map_err(|why| format!("could not load witness: {:?}", why))?;

    let pk_path = Path::new(sub_matches.value_of("provingkey").unwrap());
    let proof_path = Path::new(sub_matches.value_of("proofpath").unwrap());

    let program_path = Path::new(sub_matches.value_of("input").unwrap());
    let program_file = File::open(&program_path)
        .map_err(|why| format!("couldn't open {}: {}", program_path.display(), why))?;

    let mut reader = BufReader::new(program_file);

    let program: ir::Prog<FieldPrime> =
        deserialize_from(&mut reader, Infinite).map_err(|why| format!("{:?}", why))?;

    let pk_file = File::open(&pk_path)
        .map_err(|why| format!("couldn't open {}: {}", pk_path.display(), why))?;

    let mut pk: Vec<u8> = Vec::new();
    let mut pk_reader = BufReader::new(pk_file);
    pk_reader
        .read_to_end(&mut pk)
        .map_err(|why| format!("couldn't read {}: {}", pk_path.display(), why))?;

    let proof = scheme.generate_proof(program, witness, pk);
    let mut proof_file = File::create(proof_path).unwrap();

    proof_file
        .write(proof.as_ref())
        .map_err(|why| format!("couldn't write to {}: {}", proof_path.display(), why))?;

    println!("generate-proof successful: {}", format!("{}", proof));
}

#[no_mangle]
pub fn bl_setup<T: Field, E: Into<imports::Error>>(
    // FIXME add params
    // circuit
) -> Result { // FIXME add retval
    let scheme = get_scheme(sub_matches.value_of("proving-scheme").unwrap())?;

    println!("Performing setup...");

    let path = Path::new(sub_matches.value_of("input").unwrap());
    let file = File::open(&path)
        .map_err(|why| format!("couldn't open {}: {}", path.display(), why))?;

    let mut reader = BufReader::new(file);

    let program: ir::Prog<FieldPrime> =
        deserialize_from(&mut reader, Infinite).map_err(|why| format!("{:?}", why))?;

    // print deserialized flattened program
    if !sub_matches.is_present("light") {
        println!("{}", program);
    }

    // get paths for proving and verification keys
    let pk_path = Path::new(sub_matches.value_of("proving-key-path").unwrap());
    let vk_path = Path::new(sub_matches.value_of("verification-key-path").unwrap());

    // run setup phase
    let keypair = scheme.setup(program);

    // write verification key
    let mut vk_file = File::create(vk_path)
        .map_err(|why| format!("couldn't create {}: {}", vk_path.display(), why))?;
    vk_file
        .write(keypair.vk.as_ref())
        .map_err(|why| format!("couldn't write to {}: {}", vk_path.display(), why))?;

    // write proving key
    let mut pk_file = File::create(pk_path)
        .map_err(|why| format!("couldn't create {}: {}", pk_path.display(), why))?;
    pk_file
        .write(keypair.pk.as_ref())
        .map_err(|why| format!("couldn't write to {}: {}", pk_path.display(), why))?;

    println!("Setup completed.");
}
