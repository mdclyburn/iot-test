//! Runtime configuration options.

use std::env;
use std::fmt;
use std::fmt::Display;
use std::path::Path;

use clockwise_common::input::TestProvider;
use getopts::Options;

use crate::input::TestbedConfigReader;
use crate::input::json::JSONTestbedParser;
use crate::input::hard_code::{
    HardCodedTestbed,
    HardCodedTests,
};
use crate::input::shared_lib::LibraryTestProvider;

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Error {
    /// An option is missing its required argument.
    ArgumentMissing(&'static str),
    /// Parsing command line failed.
    CLI(getopts::Fail),
    /// User requested to see help, not run the program.
    Help(String),
    /// User passed an invalid option.
    Invalid(String),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::CLI(ref e) => Some(e),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            ArgumentMissing(arg) => write!(f, "missing argument for '{}' option", arg),
            Help(ref help_msg) => write!(f, "Program help:\n{}", help_msg),
            Invalid(ref opt) => write!(f, "Invalid option: {}", opt),
            _ => write!(f, ""),
        }
    }
}

impl From<getopts::Fail> for Error {
    fn from(e: getopts::Fail) -> Error {
        Error::CLI(e)
    }
}

#[derive(Debug)]
pub struct Configuration {
    testbed_reader: Box<dyn TestbedConfigReader>,
    test_adapter: Box<dyn TestProvider>,
}

impl Configuration {
    fn new(testbed_reader: Box<dyn TestbedConfigReader>,
           test_adapter: Box<dyn TestProvider>) -> Configuration
    {
        Configuration {
            testbed_reader,
            test_adapter,
        }
    }

    pub fn get_testbed_reader(&self) -> &dyn TestbedConfigReader {
        self.testbed_reader.as_ref()
    }

    pub fn get_test_adapter(&self) -> &dyn TestProvider {
        self.test_adapter.as_ref()
    }
}

fn create_options() -> Options {
    let mut opts = Options::new();
    opts.optopt("b", "testbed-format", "select a testbed input format", "FORMAT");
    opts.optopt("t", "test-format", "select a test input format", "FORMAT");
    opts.optflag("h", "help", "show help");

    opts
}

pub fn parse<'a>() -> Result<Configuration> {
    let opts = create_options();

    let cli_args: Vec<_> = env::args().collect();
    let matches = opts.parse(&cli_args[1..])?;
    let mut free_args = matches.free.iter();

    if matches.opt_present("h") {
        let brief = format!("Usage: {} [ options ] <config-specific options>", &cli_args[0]);
        Err(Error::Help(opts.usage(&brief)))
    } else {
        let testbed_reader: Box<dyn TestbedConfigReader> = if matches.opt_present("testbed-format") {
            let format = matches.opt_str("testbed-format")
                .ok_or(Error::ArgumentMissing("testbed-format"))?;
            match format.as_str() {
                "code" => {
                    Ok(Box::new(HardCodedTestbed::new()) as Box<dyn TestbedConfigReader>)
                },

                "json" => {
                    // Free arguments
                    let testbed_config = free_args.next()
                        .ok_or(Error::ArgumentMissing("testbed config"))?;

                    let json_path = Path::new(testbed_config);
                    Ok(Box::new(JSONTestbedParser::new(json_path)) as Box<dyn TestbedConfigReader>)
                },

                _ => {
                    let msg = format!("{} is not a testbed format", format);
                    Err(Error::Invalid(msg))
                },
            }?
        } else {
            // Default to the hard-coded testbed.
            Box::new(HardCodedTestbed::new())
        };

        let test_adapter: Box<dyn TestProvider> = if matches.opt_present("test-format") {
            let test_format = matches.opt_str("test-format")
                .ok_or(Error::ArgumentMissing("test-format"))?;
            match test_format.as_str() {
                "code" => {
                    // hard-coded test adapter
                    Ok(Box::new(HardCodedTests::new()) as Box<dyn TestProvider>)
                },

                "lib" => {
                    let library_path = free_args.next()
                        .ok_or(Error::ArgumentMissing("library path"))?;
                    let library_provider = LibraryTestProvider::new(Path::new(library_path));
                    Ok(Box::new(library_provider) as Box<dyn TestProvider>)
                }

                _ => {
                    let msg = format!("{} is not a test format", test_format);
                    Err(Error::Invalid(msg))
                },
            }?
        } else {
            // Default to the hard-coded tests.
            Box::new(HardCodedTests::new())
        };

        Ok(Configuration::new(testbed_reader, test_adapter))
    }
}
