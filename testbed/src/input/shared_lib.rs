use std::path::{Path, PathBuf};

use flexbed_common::input::TestConfigAdapter;
use flexbed_common::test::Test;
use libloading::{Library, Symbol};

#[derive(Debug)]
pub struct LibraryTestProvider {
    library_path: PathBuf,
    library: Library,
    test_adapter: Box<dyn TestConfigAdapter>,
}

impl LibraryTestProvider {
    pub fn new(path: &Path) -> LibraryTestProvider {
        let library = unsafe {
            Library::new(path)
                .expect("Failed to load library test provider's shared library.")
        };

        let test_adapter = unsafe {
            let sym: Symbol<unsafe extern fn() -> Box<dyn TestConfigAdapter>> =
                library.get(b"get_test_adapter")
                .expect("Failed to load function symbol from test provider's shared library.");

            sym()
        };

        LibraryTestProvider {
            library_path: path.to_owned(),
            library,
            test_adapter,
        }
    }
}

impl TestConfigAdapter for LibraryTestProvider {
    fn tests(&self) -> Box<dyn Iterator<Item = Test> + '_> {
        self.test_adapter.tests()
    }
}
