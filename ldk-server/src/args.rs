use clap::Parser;

#[derive(Parser, Debug)]
pub struct Cli {
	#[arg(short, long)]
	pub config: Option<String>,
}
