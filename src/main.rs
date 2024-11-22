use die::*;
use exechelper as exec_helpers;
use simple_interaction as interact;

mod blob_log_parser;
mod check;
mod cli;
mod core;
mod difflog;
mod git_helpers3;
mod repo_file;
mod split_in;
mod split_out;
mod sync;
mod topbase;
mod verify;

fn main() {
    cli::validate_input_and_run(cli::get_cli_input());
}
