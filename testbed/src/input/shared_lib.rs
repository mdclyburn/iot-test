use std::path::{Path, PathBuf};

use clockwise_common::input::{self, TestProvider, TestbedProvider};
use clockwise_common::test::Test;
use clockwise_common::testbed::Testbed;
use libloading::{Library, Symbol};

/** Shared library testbed provider.

Produces a [`TestbedProvider`] from a shared library.
During the call to [`LibraryTestbedProvider::new()`] the application loads the shared library.
The `get_testbed_provider` symbol must be a function which returns a `Box<dyn TestbedProvider>`.
 */

#[derive(Debug)]
pub struct LibraryTestbedProvider {
    library_path: PathBuf,
    library: Library,
}

impl LibraryTestbedProvider {
    /// Create a new `LibraryTestbedProvider`.
    ///
    /// This call loads the library given at `lib_path`.
    pub fn new(lib_path: &Path) -> LibraryTestbedProvider {
        let library = unsafe {
            Library::new(lib_path)
                .expect("Failed to load library testbed provider's shared library.")
        };

        LibraryTestbedProvider {
            library_path: lib_path.to_owned(),
            library,
        }
    }
}

impl TestbedProvider for LibraryTestbedProvider {
    fn create(&self) -> Result<Testbed, String> {
        unsafe {
            self.library.get(b"get_testbed")
                .map_err(|e| format!("Failed to load function symbol from testbed provider's shared library:\n{}", e))
                .and_then(|get_testbed_sym: Symbol<unsafe extern fn() -> Result<Testbed, String>>| get_testbed_sym())
        }
    }
}

/** Shared library test provider.

`LibraryTestProvider` provides an implementor of [`TestProvider`] from a shared library.
During the call to [`LibraryTestProvider::new()`] the application loads the shared library.
Then, it loads the `get_test_adapter` symbol from the library.
The `get_test_adapter` symbol must be a function which returns a `Box<dyn TestProvider>`.
 */
#[derive(Debug)]
pub struct LibraryTestProvider {
    library_path: PathBuf,
    // Implicitly suggest dropping the test adapter before letting the library unload.
    test_adapter: Box<dyn TestProvider>,
    library: Library,
}

impl LibraryTestProvider {
    pub fn new(path: &Path) -> LibraryTestProvider {
        let library = unsafe {
            Library::new(path)
                .expect("Failed to load library test provider's shared library.")
        };

        let test_adapter = unsafe {
            let sym: Symbol<unsafe extern fn() -> Box<dyn TestProvider>> =
                library.get(b"get_test_adapter")
                .expect("Failed to load function symbol from test provider's shared library.");

            sym()
        };

        LibraryTestProvider {
            library_path: path.to_owned(),
            test_adapter,
            library,
        }
    }
}

impl TestProvider for LibraryTestProvider {
    fn tests(&self) -> Box<dyn Iterator<Item = Test> + '_> {
        self.test_adapter.tests()
    }
}
