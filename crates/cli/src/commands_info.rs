pub(crate) fn info() {
    // Print information about the current environment
    println!("Rust Version: {}", env!("CARGO_PKG_VERSION"));

    // Print information about python
    #[cfg(feature = "python")]
    {
        use pyo3::prelude::*;
        use pyo3::types::{PyList, PyMapping, PyString};

        // Acquire the Global Interpreter Lock (GIL) and open a Python context
        Python::with_gil(|py| {
            // Import the sys module
            let sys = py.import("sys").unwrap();

            // Get some attributes from sys
            let version = sys.getattr("version").unwrap();
            let version = version.downcast().unwrap();
            let executable = sys.getattr("executable").unwrap();
            let executable = executable.downcast().unwrap();
            let prefix = sys.getattr("prefix").unwrap();
            let prefix = prefix.downcast().unwrap();
            let base_prefix = sys.getattr("base_prefix").unwrap();
            let base_prefix = base_prefix.downcast().unwrap();
            // Print them out or do whatever you want with them
            println!("Python Version: {}", version.to_str().unwrap());
            println!("Executable: {}", executable.to_str().unwrap());
            println!("Prefix: {}", prefix.to_str().unwrap());
            println!("Base Prefix: {}", base_prefix.to_str().unwrap());

            let sys_path = sys.getattr("path").unwrap();
            let sys_path = sys_path.downcast::<PyList>().unwrap();
            println!("sys.path:");
            for p in sys_path.iter() {
                println!("  {}", p);
            }

            // If you need to check environment variables (e.g. VIRTUAL_ENV),
            // you can import "os" and query os.environ:
            let os = py.import("os").unwrap();
            let environ = os.getattr("environ").unwrap();
            let environ = environ.downcast::<PyMapping>().unwrap();
            // If VIRTUAL_ENV is set, you can get it like:
            if let Ok(environ) = environ.get_item("VIRTUAL_ENV") {
                // if string
                let virtual_env = environ.downcast::<PyString>();
                if let Ok(virtual_env) = virtual_env {
                    println!("VIRTUAL_ENV: {}", virtual_env);
                } else {
                    println!("VIRTUAL_ENV not set.");
                }
            } else {
                println!("VIRTUAL_ENV not set.");
            }

            Ok::<_, String>(())
        })
        .unwrap();
    }
}
