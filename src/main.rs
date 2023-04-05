use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Seek, Write};
use std::time::Duration;
use std::{fmt, io};
use std::process::Command;
use std::str::FromStr;

use clap::{arg, Arg, ArgAction};
use crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use indicatif::ProgressBar;
use openai::completions::Completion;
use rustpython::vm;
use rustpython::vm::PyObjectRef;
use tempfile::NamedTempFile;
use tokio::signal::unix::{signal, SignalKind};
use toml::Value;

/*
TODO: Export program to a script that also accepts piped input or a file as input.
*/

#[tokio::main]
async fn main() {
    let mut ctrl_c = signal(SignalKind::interrupt()).expect("Error setting Ctrl+C handler");

    let ctrl_c_fut = async {
        ctrl_c.recv().await;
        eprintln!();
        eprintln!("Caught Ctrl+C; exiting.");
        std::process::exit(0);
    };

    let key = match read_or_create_config() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Error reading config file: {}", e);
            std::process::exit(1);
        }
    };
    openai::set_key(key);

    let args = parse_command_line_arguments();

    let input = read_input(args.input_file.as_deref());

    let program_fut = execute_program_loop(&input, args);

    tokio::select! {
        _ = ctrl_c_fut => {}
        _ = program_fut => {}
    }
}

struct Arguments {
    task: String,
    temperature: f32,
    max_tokens: u16,
    jsonify: bool,
    jsonify_one_line: bool,
    input_file: Option<String>,
    show_lines: Option<u16>,
}

fn parse_command_line_arguments() -> Arguments {
    let matches = clap::Command::new("GPT text processing assistant")
        .version("1.0")
        .arg_required_else_help(true)
        .arg(arg!(<task> "Description of a text processing task"))
        .arg(
            Arg::new("task")
                .index(1)
                .required(true)
                .help("Description of a text processing task"),
        )
        .arg(
            Arg::new("temperature")
                .long("temperature")
                .short('t')
                .default_value("0.25")
                .value_parser(f32::from_str)
                .help("Set GPT randomness (0 is deterministic)"),
        )
        .arg(
            Arg::new("max-tokens")
                .long("max-tokens")
                .short('m')
                .default_value("512")
                .value_parser(u16::from_str)
                .help("Set GPT response token limit"),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .short('j')
                .action(ArgAction::SetTrue)
                .help("Serialize program output to JSON"),
        )
        .arg(
            Arg::new("json-one-line")
                .long("json-one-line")
                .action(ArgAction::SetTrue)
                .help("Serialize JSON output to one line (requires --json)"),
        )
        .arg(
            Arg::new("input")
                .long("input")
                .short('i')
                .help("Read data from a file instead of STDIN"),
        )
        .arg(
            Arg::new("show-lines")
                .long("show-lines")
                .short('s')
                .value_parser(u16::from_str)
                .help("Show GPT the first N lines of the input to help it generate the program"),
        )
        .get_matches();

    let task = matches.get_one::<String>("task").unwrap();
    let temperature = matches.get_one::<f32>("temperature").unwrap();
    let max_tokens = matches.get_one::<u16>("max-tokens").unwrap();
    let jsonify = matches.get_flag("json");
    let jsonify_one_line = matches.get_flag("json-one-line");
    let input_file = matches.get_one::<String>("input");
    let show_lines = matches.get_one::<u16>("show-lines");

    validate_json_flags(jsonify, jsonify_one_line);

    Arguments {
        task: task.clone(),
        temperature: *temperature,
        max_tokens: *max_tokens,
        jsonify: jsonify,
        jsonify_one_line: jsonify_one_line,
        input_file: input_file.cloned(),
        show_lines: show_lines.cloned(),
    }
}

fn validate_json_flags(jsonify: bool, jsonify_one_line: bool) {
    if jsonify_one_line && !jsonify {
        eprintln!("Error: --json-one-line requires --json to be set.");
        std::process::exit(1);
    }
}

fn read_input(input_file: Option<&str>) -> String {
    match input_file {
        Some(file) => read_file_input(file),
        None => read_piped_input(),
    }
}

fn read_file_input(file: &str) -> String {
    let mut input = String::new();
    if let Ok(mut file) = File::open(file) {
        file.read_to_string(&mut input).unwrap_or_else(|e| {
            eprintln!("Error reading input file: {}", e);
            std::process::exit(1);
        });
    } else {
        eprintln!("Error opening input file: {}", file);
        std::process::exit(1);
    }
    input
}

fn read_piped_input() -> String {
    let mut input = String::new();
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    match handle.read_to_string(&mut input) {
        Ok(_) => {}
        Err(e) => eprintln!("Error reading piped input: {}", e),
    }
    input
}

const TICK_INTERVAL: u64 = 100;

async fn execute_program_loop(input: &str, args: Arguments) {
    #[inline(always)]
    async fn generate_program_with_progress(args: &Arguments, input: &str) -> String {
        let pb = ProgressBar::new_spinner();
        pb.set_message("Generating program...");
        pb.enable_steady_tick(Duration::from_millis(TICK_INTERVAL));
        let program = generate_program(
            &args.task,
            args.temperature,
            args.max_tokens,
            args.jsonify,
            args.jsonify_one_line,
            args.show_lines,
            input,
        )
            .await
            .unwrap();
        pb.finish_and_clear();
        program
    }

    fn prompt_for_run_program() -> char {
        prompt("Run generated program? ([y]es/[q]uit/[r]egen/[e]dit) ")
    }

    fn show_generated_program(program: &str, edited: &mut bool) {
        if !*edited {
            eprintln!("Generated program:");
        } else {
            eprintln!("Edited program:");
            *edited = false;
        }
        eprintln!("------------------------------");
        eprintln!("{}", program);
        eprintln!("------------------------------");
    }

    //

    let mut program = generate_program_with_progress(&args, input).await;
    let mut program_hist = vec![program.clone()];
    let mut edited = false;

    loop {
        show_generated_program(&program, &mut edited);

        match prompt_for_run_program() {
            'y' => {
                eprintln!();
                match execute_program(input, program).await {
                    Ok(v) => {
                        println!("{}", v);
                        break;
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        if prompt("Do you want to regenerate the program? ([y]es/[n]o) ") == 'y' {
                            program = generate_program_with_progress(&args, input).await;
                            if program_hist.contains(&program) {
                                eprintln!("Re-generated program is identical to previously generated program. Please rephrase your task.");
                                break;
                            } else {
                                program_hist.push(program.clone());
                                continue;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
            'q' => {
                break;
            }
            'r' => {
                eprintln!();
                program = generate_program_with_progress(&args, input).await;
                if program_hist.contains(&program) {
                    eprintln!("Re-generated program is identical to previously generated program. Please rephrase your task.");
                    break;
                } else {
                    program_hist.push(program.clone());
                }
            }
            'e' => {
                eprintln!();
                match edit_program_with_vi(&program) {
                    Ok(edited_program) => {
                        program = edited_program;
                        edited = true;
                    }
                    Err(e) => {
                        eprintln!("Error editing program with 'vi': {}", e);
                    }
                }
            }
            _ => {
                eprintln!("Invalid input; enter 'y', 'q', 'r', or 'e'.");
                continue;
            }
        }
    }
}

fn edit_program_with_vi(program: &str) -> Result<String, Box<dyn Error>> {
    // Create a temporary file and write the program to it
    let mut temp_file = NamedTempFile::new()?;
    temp_file.write_all(program.as_bytes())?;

    // Open the temporary file with the 'vi' editor
    let status = Command::new("vi")
        .arg(temp_file.path())
        .status()?;

    // Check if the 'vi' command exited successfully
    if !status.success() {
        return Err(format!("'vi' command exited with an error: {}", status).into());
    }

    // Read the edited content back from the temporary file
    let mut edited_program = String::new();
    temp_file.seek(io::SeekFrom::Start(0))?;
    temp_file.read_to_string(&mut edited_program)?;

    Ok(edited_program)
}

fn read_or_create_config() -> Result<String, Box<dyn Error>> {
    let config_dir = dirs::config_dir().ok_or("Unable to find config directory")?;
    let config_path = config_dir.join("gptxt.toml");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    if !config_path.exists() {
        let mut file = File::create(&config_path)?;
        file.write_all(br#"key = """#)?;
        eprintln!(
            "Created a new configuration file at: {}",
            config_path.display()
        );
        eprintln!("Set the 'key' value in the file before using the program.");
        std::process::exit(1);
    }

    let contents = fs::read_to_string(&config_path)?;
    let config: Value = contents.parse()?;

    let key = match config.get("key") {
        Some(key) => key.as_str().unwrap_or("").to_string(),
        None => {
            eprintln!(
                "The 'key' value is not set in the configuration file: {}",
                config_path.display()
            );
            std::process::exit(1);
        }
    };

    if key.is_empty() {
        eprintln!(
            "Set the 'key' value in the configuration file before using the program: {}",
            config_path.display()
        );
        std::process::exit(1);
    }

    Ok(key)
}

const SYSTEM_MESSAGE: &str =
    "# You are part of an API that creates Python program for text processing.
# You should return only Python program with no comments.
# Do not describe the program or add any additional information about the program.
# Data to process is stored in the string variable `data`.
# Results should be stored in the variable `result`.

import sys
data = sys.stdin.read()
";

async fn generate_program(
    task: &str,
    temperature: f32,
    max_tokens: u16,
    jsonify: bool,
    jsonify_one_line: bool,
    show_lines: Option<u16>,
    input: &str,
) -> Result<String, Box<dyn Error>> {
    let mut prompt = SYSTEM_MESSAGE.to_owned();

    if let Some(n) = show_lines {
        let shown_lines = input
            .lines()
            .take(n as usize)
            .map(|s| format!("#>{}", s))
            .collect::<Vec<String>>()
            .join("\n");

        prompt.push_str(&format!(
            "\n# First {} lines of `data`:\n{}\n",
            n, shown_lines
        ));
    }

    prompt.push_str(&format!("\n# {}:", task));

    //

    let completion = Completion::builder("text-davinci-003")
        .prompt(&prompt)
        .temperature(temperature)
        .max_tokens(max_tokens)
        .create()
        .await?;

    match completion {
        Ok(completion_result) => {
            let mut program = completion_result
                .choices
                .first()
                .unwrap()
                .text
                .trim()
                .to_owned();

            if jsonify_one_line {
                program = format!(
                    "{}\nimport json; result = json.dumps(result, separators=(',', ':'))",
                    program
                );
            } else if jsonify {
                program = format!("{}\nimport json; result = json.dumps(result)", program);
            }
            Ok(program)
        }
        Err(error) => Err(Box::new(error)),
    }
}

fn prompt(message: &str) -> char {
    eprint!("{}", message);
    io::stderr().flush().unwrap();

    let input: char;

    terminal::enable_raw_mode().unwrap();

    loop {
        if let Ok(true) = poll(Duration::from_millis(100)) {
            if let Ok(Event::Key(KeyEvent {
                                     code, modifiers, ..
                                 })) = read()
            {
                match code {
                    KeyCode::Char(ch @ 'y') | KeyCode::Char(ch @ 'q') | KeyCode::Char(ch @ 'r') | KeyCode::Char(ch @ 'e') => {
                        input = ch;
                        break;
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        terminal::disable_raw_mode().unwrap();
                        eprintln!("Caught Ctrl+C; exiting.");
                        std::process::exit(0);
                    }
                    KeyCode::Char('\\') if modifiers.contains(KeyModifiers::CONTROL) => {
                        terminal::disable_raw_mode().unwrap();
                        eprintln!(r#"Caught Ctrl+\; exiting."#);
                        std::process::exit(0);
                    }
                    _ => {
                        io::stderr().flush().unwrap();
                    }
                }
            }
        }
    }

    terminal::disable_raw_mode().unwrap();

    eprintln!("{}", input);
    input
}

#[derive(Debug)]
enum ExecuteError {
    CompileError(String),
    ExecutionError(String),
    ResultNotFound,
    ResultConversionError(String),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExecuteError::CompileError(err) => write!(f, "Error compiling Python program: {}", err),
            ExecuteError::ExecutionError(err) => {
                write!(f, "Error executing Python program: {}", err)
            }
            ExecuteError::ResultNotFound => write!(f, "Error: 'result' variable not found"),
            ExecuteError::ResultConversionError(t) => write!(
                f,
                "Error: Failed to convert 'result' PyObject to a Rust String; type is: {}",
                t
            ),
        }
    }
}

async fn execute_program(input: &str, returned_message: String) -> Result<String, ExecuteError> {
    let interp = rustpython::InterpreterConfig::new()
        .init_stdlib()
        .interpreter();

    interp.enter(|vm| {
        let program_obj = vm
            .compile(
                returned_message.as_ref(),
                vm::compiler::Mode::Exec,
                "<string>".to_owned(),
            )
            .map_err(|err| ExecuteError::CompileError(err.to_string()))?;

        let scope = vm.new_scope_with_builtins();

        let data_pyobj = vm.ctx.new_str(input);
        scope
            .locals
            .set_item("data", PyObjectRef::from(data_pyobj), vm)
            .expect("Failed to set variable in scope");

        vm.run_code_obj(program_obj, scope.clone()).map_err(|err| {
            let mut buf = String::new();
            vm.write_exception(&mut buf, &err)
                .expect("Failed to write exception");
            ExecuteError::ExecutionError(buf)
        })?;

        let result_pyobj = scope
            .locals
            .get_item("result", vm)
            .map_err(|_| ExecuteError::ResultNotFound)?;

        let result_str: String = result_pyobj.clone().try_into_value(vm).map_err(|_| {
            let n = result_pyobj.clone().class().name().to_owned();
            ExecuteError::ResultConversionError(n)
        })?;

        let result_norm = result_str.replace(r#"\r"#, "\r").replace(r#"\n"#, "\n");

        Ok(result_norm)
    })
}
